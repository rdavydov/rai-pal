use std::collections::HashSet;

use steamlocate::SteamDir;

use super::appinfo::{
	self,
	SteamAppInfoFile,
};
use crate::{
	game::OperatingSystem,
	serializable_enum,
	serializable_struct,
	Result,
};

const UNITY_STEAM_APP_IDS_URL: &str =
	"https://raw.githubusercontent.com/Raicuparta/steam-unity-app-ids/main";

serializable_enum!(GameEngine {
	Unity,
	Unreal,
	Godot,
});

serializable_struct!(OwnedGame {
	id: String,
	name: String,
	installed: bool,
	os_list: HashSet<OperatingSystem>,
	engine: GameEngine,
	release_date: i32,
});

async fn get_engine_games(
	engine: GameEngine,
	steam_dir: &SteamDir,
	app_info: &SteamAppInfoFile,
) -> Result<Vec<OwnedGame>> {
	let response = reqwest::get(format!("{UNITY_STEAM_APP_IDS_URL}/{engine}"))
		.await?
		.text()
		.await?;

	Ok(response
		.split('\n')
		.filter_map(|app_id| {
			let id = app_id.parse::<u32>().ok()?;

			let app_info = app_info.apps.get(&id)?;

			let os_list: HashSet<_> = app_info
				.launch_options
				.iter()
				.filter_map(|launch| match launch.clone().os_list?.as_str() {
					"linux" => Some(OperatingSystem::Linux),
					"windows" => Some(OperatingSystem::Windows),
					_ => None,
				})
				.collect();

			let installed = steam_dir
				.app(id)
				.map_or(false, |steam_app| steam_app.is_some());

			let release_date = app_info
				.original_release_date
				.or(app_info.steam_release_date)
				.unwrap_or_default();

			Some(OwnedGame {
				id: app_id.to_owned(),
				name: app_info.name.clone(),
				installed,
				os_list,
				engine,
				release_date,
			})
		})
		.collect())
}

pub async fn get() -> Result<Vec<OwnedGame>> {
	let steam_dir = SteamDir::locate()?;
	let app_info = appinfo::read(&steam_dir.path().join("appcache/appinfo.vdf"))?;

	Ok([
		get_engine_games(GameEngine::Unity, &steam_dir, &app_info).await?,
		get_engine_games(GameEngine::Unreal, &steam_dir, &app_info).await?,
		get_engine_games(GameEngine::Godot, &steam_dir, &app_info).await?,
	]
	.concat())
}
