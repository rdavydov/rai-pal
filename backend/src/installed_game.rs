use std::{
	collections::HashMap,
	fs::{self,},
	path::{
		Path,
		PathBuf,
	},
};

use crate::{
	game_executable::GameExecutable,
	mod_loaders::mod_loader,
	paths::{self,},
	providers::provider::ProviderId,
	serializable_struct,
	steam::{
		self,
		appinfo::SteamLaunchOption,
	},
	Result,
};

serializable_struct!(InstalledGame {
	pub name: String,
	pub provider_id: ProviderId,
	pub discriminator: Option<String>,
	pub steam_launch: Option<SteamLaunchOption>,
	pub executable: GameExecutable,
	pub thumbnail_url: Option<String>,
	pub available_mods: HashMap<String, bool>,
});

pub type Map = HashMap<PathBuf, InstalledGame>;

impl InstalledGame {
	pub fn new(
		path: &Path,
		name: &str,
		provider_id: &ProviderId,
		discriminator: Option<String>,
		steam_launch: Option<&SteamLaunchOption>,
		thumbnail_url: Option<String>,
		mod_loaders: &mod_loader::DataMap,
	) -> Option<Self> {
		// Games exported by Unity always have one of these extensions.
		const VALID_EXTENSIONS: [&str; 3] = ["exe", "x86_64", "x86"];

		if !path.is_file() {
			return None;
		}

		// We ignore games that don't have an extension.
		let extension = path.extension()?.to_str()?;

		if !VALID_EXTENSIONS.contains(&extension) {
			return None;
		}

		if extension == "x86" && path.with_extension("x86_64").is_file() {
			// If there's an x86_64 version, we ignore the x86 version.
			// I'm just gonna presume there are no x86 modders out there,
			// if someone cries about it I'll make this smarter.
			return None;
		}

		let executable = GameExecutable::new(path);
		let available_mods = executable.get_available_mods(mod_loaders);

		Some(Self {
			name: name.to_string(),
			provider_id: provider_id.to_owned(),
			discriminator,
			steam_launch: steam_launch.cloned(),
			available_mods,
			executable,
			thumbnail_url,
		})
	}

	pub fn open_game_folder(&self) -> Result {
		Ok(open::that_detached(paths::path_parent(
			&self.executable.path,
		)?)?)
	}

	pub fn get_installed_mods_folder(&self) -> Result<PathBuf> {
		get_installed_mods_folder(&self.executable.path)
	}

	pub fn open_mods_folder(&self) -> Result {
		Ok(open::that_detached(self.get_installed_mods_folder()?)?)
	}

	pub fn start(&self, handle: &tauri::AppHandle) -> Result {
		self.steam_launch.as_ref().map_or_else(
			|| Ok(open::that_detached(&self.executable.path)?),
			|steam_launch| {
				if self.discriminator.is_none() {
					// If a game has no discriminator, it means we're probably using the default launch option.
					// For those, we use the steam://rungameid command, since that one will make steam show a nice
					// loading popup, wait for game updates, etc.
					return steam::command::run(
						&format!("rungameid/{}", steam_launch.app_id),
						handle,
					);
				}
				// For the few cases where we're showing an alternative launch option, we use the steam://launch command.
				// This one will show an error if the game needs an update, and doesn't show the nice loading popup,
				// but it allows us to specify the specific launch option to run.
				// This one also supports passing "dialog" instead of the app_type, (steam://launch/{app_id}/dialog)
				// which makes Steam show the launch selection dialogue, but that dialogue stops showing if the user
				// selects the "don't ask again" checkbox.
				steam::command::run(
					&format!(
						"launch/{}/{}",
						steam_launch.app_id,
						steam_launch.app_type.as_deref().unwrap_or("")
					),
					handle,
				)
			},
		)
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

	pub fn refresh_mods(&mut self, mod_loaders: &mod_loader::DataMap) {
		self.available_mods = self.executable.get_available_mods(mod_loaders);
	}
}

fn get_installed_mods_folder(game_path: &Path) -> Result<PathBuf> {
	let installed_mods_folder = paths::app_data_path()?.join("games").join(game_path);
	fs::create_dir_all(&installed_mods_folder)?;

	Ok(installed_mods_folder)
}
