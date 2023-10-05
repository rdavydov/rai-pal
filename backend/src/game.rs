use std::{
	collections::HashMap,
	fs::{
		self,
		metadata,
		File,
	},
	io::Read,
	path::{
		Path,
		PathBuf,
	},
};

use goblin::{
	elf::Elf,
	pe::PE,
};
use lazy_regex::{
	regex_captures,
	regex_find,
};

use crate::{
	paths::{
		self,
		glob_path,
	},
	serializable_enum,
	serializable_struct,
	steam::appinfo::SteamLaunchOption,
	Error,
	Result,
};

serializable_enum!(UnityScriptingBackend {
	Il2Cpp,
	Mono,
	Unknown
});
serializable_enum!(Architecture { Unknown, X64, X86 });
serializable_enum!(OperatingSystem {
	Unknown,
	Linux,
	Windows
});

serializable_enum!(GameEngineBrand {
	Unity,
	Unreal,
	Godot,
});

serializable_struct!(GameEngine {
	pub brand: GameEngineBrand,
	pub version: Option<GameEngineVersion>,
});

serializable_struct!(GameEngineVersion {
	pub major: u32,
	pub minor: u32,
	pub patch: u32,
	pub suffix: Option<String>,
	pub display: String,
});

serializable_struct!(Game {
	pub id: String,
	pub name: String,
	pub discriminator: Option<String>,
	pub full_path: PathBuf,
	pub architecture: Architecture,
	pub scripting_backend: UnityScriptingBackend,
	pub operating_system: OperatingSystem,
	pub steam_launch: Option<SteamLaunchOption>,
	pub installed_mods: Vec<String>,
	pub engine: Option<GameEngine>,
	pub thumbnail_url: Option<String>,
});

pub type Map = HashMap<String, Game>;

impl Game {
	pub fn new(
		id: &str,
		name: &str,
		discriminator: Option<String>,
		full_path: &Path,
		steam_launch: Option<&SteamLaunchOption>,
		thumbnail_url: Option<String>,
	) -> Option<Self> {
		// Games exported by Unity always have one of these extensions.
		const VALID_EXTENSIONS: [&str; 3] = ["exe", "x86_64", "x86"];

		// We ignore games that don't have an extension.
		let extension = full_path.extension()?.to_str()?;

		if !VALID_EXTENSIONS.contains(&extension) {
			return None;
		}

		if extension == "x86" && full_path.with_extension("x86_64").is_file() {
			// If there's an x86_64 version, we ignore the x86 version.
			// I'm just gonna presume there are no x86 modders out there,
			// if someone cries about it I'll make this smarter.
			return None;
		}

		let (operating_system, architecture) = get_os_and_architecture(full_path).ok()?;

		let installed_mods = match get_installed_mods(id) {
			Ok(mods) => mods,
			Err(err) => {
				println!("Failed to get installed mods for game {id}: {err}");
				vec![]
			}
		};

		let engine = get_engine(full_path);
		let scripting_backend = engine.as_ref().map_or(UnityScriptingBackend::Unknown, |e| {
			if e.brand == GameEngineBrand::Unity {
				get_unity_scripting_backend(full_path).unwrap_or(UnityScriptingBackend::Unknown)
			} else {
				UnityScriptingBackend::Unknown
			}
		});

		Some(Self {
			architecture,
			full_path: full_path.to_path_buf(),
			id: id.to_string(),
			operating_system,
			name: name.to_string(),
			discriminator,
			scripting_backend,
			steam_launch: steam_launch.cloned(),
			installed_mods,
			engine,
			thumbnail_url,
		})
	}

	pub fn open_game_folder(&self) -> Result {
		Ok(open::that_detached(paths::path_parent(&self.full_path)?)?)
	}

	pub fn get_installed_mods_folder(&self) -> Result<PathBuf> {
		get_installed_mods_folder(&self.id)
	}

	pub fn open_mods_folder(&self) -> Result {
		Ok(open::that_detached(self.get_installed_mods_folder()?)?)
	}

	pub fn start(&self) -> Result {
		Ok(self.steam_launch.as_ref().map_or_else(
			|| open::that_detached(&self.full_path),
			|steam_launch| {
				if self.discriminator.is_none() {
					// If a game has no discriminator, it means we're probably using the default launch option.
					// For those, we use the steam://rungameid command, since that one will make steam show a nice
					// loading popup, wait for game updates, etc.
					return open::that_detached(format!(
						"steam://rungameid/{}",
						steam_launch.app_id
					));
				}
				// For the few cases where we're showing an alternative launch option, we use the steam://launch command.
				// This one will show an error if the game needs an update, and doesn't show the nice loading popup,
				// but it allows us to specify the specific launch option to run.
				// This one also supports passing "dialog" instead of the app_type, (steam://launch/{app_id}/dialog)
				// which makes Steam show the launch selection dialogue, but that dialogue stops showing if the user
				// selects the "don't ask again" checkbox.
				open::that_detached(format!(
					"steam://launch/{}/{}",
					steam_launch.app_id,
					steam_launch.app_type.as_deref().unwrap_or("")
				))
			},
		)?)
	}

	pub fn uninstall_mod(&self, mod_id: &str) -> Result {
		let installed_mods_folder = self.get_installed_mods_folder()?;
		let mod_files_folder = installed_mods_folder
			.join("BepInEx")
			.join("plugins")
			.join(mod_id);

		if mod_files_folder.is_dir() {
			fs::remove_dir_all(mod_files_folder)?;
		}

		Ok(())
	}
}

fn is_unity_exe(game_path: &Path) -> bool {
	game_path.is_file()
		&& get_unity_data_path(game_path).map_or(false, |data_path| data_path.is_dir())
}

fn is_unreal_exe(game_path: &Path) -> bool {
	const VALID_FOLDER_NAMES: [&str; 3] = ["Win64", "Win32", "ThirdParty"];

	if let Some(parent) = game_path.parent() {
		// For cases where the registered exe points to a launcher at the root level:
		if VALID_FOLDER_NAMES.iter().any(|folder_name| {
			parent
				.join("Engine")
				.join("Binaries")
				.join(folder_name)
				.is_dir()
		}) {
			return true;
		}

		// For cases where the registered exe points directly to the shipping binary:
		if parent.ends_with("Win64") || parent.ends_with("Win32") {
			if let Some(binaries) = parent.parent() {
				if binaries.ends_with("Binaries") {
					return true;
				}
			}
		}
	}

	false
}

fn get_engine(game_path: &Path) -> Option<GameEngine> {
	if is_unity_exe(game_path) {
		Some(GameEngine {
			brand: GameEngineBrand::Unity,
			version: get_unity_version(game_path),
		})
	} else if is_unreal_exe(game_path) {
		Some(GameEngine {
			brand: GameEngineBrand::Unreal,
			version: get_unreal_version(game_path),
		})
	} else {
		None
	}
}

fn get_unity_scripting_backend(game_exe_path: &Path) -> Result<UnityScriptingBackend> {
	let game_folder = paths::path_parent(game_exe_path)?;

	if game_folder.join("GameAssembly.dll").is_file()
		|| game_folder.join("GameAssembly.so").is_file()
	{
		Ok(UnityScriptingBackend::Il2Cpp)
	} else {
		Ok(UnityScriptingBackend::Mono)
	}
}

fn get_unity_data_path(game_exe_path: &Path) -> Result<PathBuf> {
	let parent = paths::path_parent(game_exe_path)?;
	let file_stem = paths::file_name_without_extension(game_exe_path)?;

	Ok(parent.join(format!("{file_stem}_Data")))
}

fn get_os_and_architecture(file_path: &Path) -> Result<(OperatingSystem, Architecture)> {
	fs::read(file_path).map(|file| {
		let elf_result = match Elf::parse(&file) {
			Ok(elf) => match elf.header.e_machine {
				goblin::elf::header::EM_X86_64 => Ok((OperatingSystem::Linux, Architecture::X64)),
				goblin::elf::header::EM_386 => Ok((OperatingSystem::Linux, Architecture::X86)),
				_ => Ok((OperatingSystem::Linux, Architecture::Unknown)),
			},
			Err(err) => Err(err),
		};

		if elf_result.is_ok() {
			return Ok(elf_result?);
		}

		let pe_result = match PE::parse(&file) {
			Ok(pe) => match pe.header.coff_header.machine {
				goblin::pe::header::COFF_MACHINE_X86_64 => {
					Ok((OperatingSystem::Windows, Architecture::X64))
				}
				goblin::pe::header::COFF_MACHINE_X86 => {
					Ok((OperatingSystem::Windows, Architecture::X86))
				}
				_ => Ok((OperatingSystem::Windows, Architecture::Unknown)),
			},
			Err(err) => Err(err),
		};

		if pe_result.is_ok() {
			return Ok(pe_result?);
		}

		println!("Failed to parse exe as ELF or PE");
		if let Err(err) = elf_result {
			println!("ELF error: {err}");
		}
		if let Err(err) = pe_result {
			println!("PE error: {err}");
		}

		Ok((OperatingSystem::Unknown, Architecture::Unknown))
	})?
}

fn get_unity_version(game_exe_path: &Path) -> Option<GameEngineVersion> {
	const ASSETS_WITH_VERSION: [&str; 3] = ["globalgamemanagers", "mainData", "data.unity3d"];

	if let Ok(data_path) = get_unity_data_path(game_exe_path) {
		for asset_name in &ASSETS_WITH_VERSION {
			let asset_path = data_path.join(asset_name);

			if let Ok(metadata) = metadata(&asset_path) {
				if metadata.is_file() {
					if let Ok(version) = get_version_from_asset(&asset_path) {
						let mut version_parts = version.split('.');
						let major = version_parts.next().unwrap_or("0").parse().unwrap_or(0);
						let minor = version_parts.next().unwrap_or("0").parse().unwrap_or(0);
						let patch = version_parts.next().unwrap_or("0").parse().unwrap_or(0);
						let suffix = version_parts.next().unwrap_or("0").to_string();

						return Some(GameEngineVersion {
							major,
							minor,
							patch,
							suffix: Some(suffix),
							display: version,
						});
					}
				}
			}
		}
	}

	None
}

fn get_actual_unreal_binary(game_exe_path: &Path) -> PathBuf {
	if let Some(parent) = game_exe_path.parent() {
		if parent.ends_with("Win64") {
			return game_exe_path.to_path_buf();
		}

		let paths = glob_path(
			&parent
				.join("*")
				.join("Binaries")
				.join("Win64")
				.join("*.exe"),
		);

		if let Ok(mut paths) = paths {
			let path = paths.find(|path_result| {
				path_result
					.as_ref()
					.map_or(false, |path| !path.starts_with(parent.join("Engine")))
			});

			if let Some(Ok(path)) = path {
				return path;
			}
		}
	}

	game_exe_path.to_path_buf()
}

fn get_unreal_version(game_exe_path: &Path) -> Option<GameEngineVersion> {
	let actual_binary = get_actual_unreal_binary(game_exe_path);
	match fs::read(actual_binary) {
		Ok(file_bytes) => {
			// Looking for strings like "++UE4+release-4.25".
			// The extra \x00 are because the strings are unicode.
			// The {0,100} is matching the "+release-" etc part,
			// it can be different for every game, but I'm limiting it to 100 chars.
			let match_result = regex_find!(
				r"(?i)\+\x00\+\x00U\x00E\x00[45]\x00.{0,100}?([45]\x00\.\x00(\d\x00)+)"B,
				&file_bytes
			)
			// Some games don't have the full version string,
			// so we try getting just the major version from strings like "+ue4"
			.or(regex_find!(r"(?i)\+\x00U\x00E\x00[45]\x00"B, &file_bytes));
			// I also noticed the game ABZU has the version in the exe as "4.12.5-0+UE4".
			// But I don't know if any other games do that. This regex would match that:
			// r"([4|5]\x00\.\x00(\d\x00)+).{0,100}?(?i)\+\x00U\x00E\x00[4|5]\x00"B

			let match_string = String::from_utf16_lossy(
				&match_result?
					.chunks(2)
					.map(|e| u16::from_le_bytes(e.try_into().unwrap_or_default()))
					.collect::<Vec<_>>(),
			);

			// Regex again because the byte regex above can't extract the match groups.
			// Full version is something like "4.25"
			let version = if let Some(full_version) = regex_find!(r"[45]\.\d+", &match_string) {
				full_version
			} else {
				// Not so full version is something like "4"
				regex_captures!(r"(?i)\+UE([45])", &match_string)?.1
			};

			// Splitting the regex result to get each part of the version.
			// I should probably just do it all in one regex tbh but I'm bad at regex.
			let version_parts: Vec<_> = version.split('.').collect();
			let major = version_parts.first().map_or(0, |f| f.parse().unwrap_or(0));
			let minor = version_parts.get(1).map_or(0, |f| f.parse().unwrap_or(0));

			return Some(GameEngineVersion {
				major,
				minor,
				patch: 0,
				suffix: None,
				display: version.to_string(),
			});
		}
		Err(err) => {
			println!("Failed to read game exe: {err}");
		}
	}

	None
}

fn get_version_from_asset(asset_path: &Path) -> Result<String> {
	let mut file = File::open(asset_path)?;
	let mut data = vec![0u8; 4096];

	let bytes_read = file.read(&mut data)?;
	if bytes_read == 0 {
		return Err(Error::EmptyFile(asset_path.to_path_buf()));
	}

	let data_str = String::from_utf8_lossy(&data[..bytes_read]);
	let match_result = regex_find!(r"\d+\.\d+\.\d+[fp]\d+", &data_str);

	match_result.map_or_else(
		|| Ok("No version found".to_string()),
		|matched| Ok(matched.to_string()),
	)
}

fn get_installed_mods_folder(id: &str) -> Result<PathBuf> {
	let installed_mods_folder = paths::app_data_path()?.join("games").join(id);
	fs::create_dir_all(&installed_mods_folder)?;

	Ok(installed_mods_folder)
}

fn get_installed_mods(id: &str) -> Result<Vec<String>> {
	let pattern = get_installed_mods_folder(id)?
		.join("BepInEx")
		.join("plugins")
		.join("*");
	let entries: Vec<_> = paths::glob_path(&pattern)?.collect();

	Ok(entries
		.iter()
		.filter_map(|entry| match entry {
			Ok(mod_path) => Some(mod_path.file_name()?.to_str()?.to_string()),
			Err(_) => None,
		})
		.collect())
}
