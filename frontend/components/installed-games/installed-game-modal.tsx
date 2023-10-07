import { Alert, Button, Flex, Modal, Stack } from "@mantine/core";
import { useModLoaders } from "@hooks/use-backend-data";
import {
	Game,
	installMod,
	openGameFolder,
	openGameModsFolder,
	startGame,
	uninstallMod,
} from "@api/bindings";
import { Fragment, useMemo, useState } from "react";
import { GameName } from "./game-name";
import { CommandButton } from "@components/command-button";
import {
	IconBooks,
	IconBrowser,
	IconDownload,
	IconFolder,
	IconFolderCog,
	IconPlayerPlay,
	IconTrash,
} from "@tabler/icons-react";
import { CodeHighlight } from "@mantine/code-highlight";
import { steamCommands } from "../../util/steam";
import { ModalImage } from "@components/modal-image";

type Props = {
	readonly game: Game;
	readonly onClose: () => void;
	readonly refreshGame: (gameId: string) => void;
};

export function InstalledGameModal(props: Props) {
	const [modLoaderMap] = useModLoaders();
	const [error, setError] = useState("");

	const handleError = (error: unknown) => {
		setError(`${error}`);
	};

	const debugData = useMemo(
		() => JSON.stringify(props.game, null, 2),
		[props.game],
	);

	const modLoaders = useMemo(
		() =>
			Object.values(modLoaderMap).map((modLoader) => ({
				...modLoader,
				mods: modLoader.mods.filter(
					(mod) => mod.scriptingBackend === props.game.scriptingBackend,
				),
			})),
		[modLoaderMap, props.game.scriptingBackend],
	);

	return (
		<Modal
			centered
			onClose={props.onClose}
			opened
			size="lg"
			title={
				<Flex>
					<GameName game={props.game} />
				</Flex>
			}
		>
			<Stack>
				<ModalImage src={props.game.thumbnailUrl} />
				{error ? (
					<Alert
						color="red"
						style={{ overflow: "auto", flex: 1 }}
					>
						<pre>{error}</pre>
					</Alert>
				) : null}
				<Button.Group orientation="vertical">
					<CommandButton
						leftSection={<IconPlayerPlay />}
						onClick={() => startGame(props.game.id)}
					>
						Start Game
					</CommandButton>
					<CommandButton
						leftSection={<IconFolder />}
						onClick={() => openGameFolder(props.game.id)}
					>
						Open Game Folder
					</CommandButton>
					<CommandButton
						leftSection={<IconFolderCog />}
						onClick={() => openGameModsFolder(props.game.id).catch(handleError)}
					>
						Open Mods Folder
					</CommandButton>
					{props.game.steamLaunch && (
						<>
							<CommandButton
								leftSection={<IconBooks />}
								onClick={() =>
									steamCommands.showInLibrary(props.game.steamLaunch?.appId)
								}
							>
								Show in Library
							</CommandButton>
							<CommandButton
								leftSection={<IconBrowser />}
								onClick={() =>
									steamCommands.openStorePage(props.game.steamLaunch?.appId)
								}
							>
								Open Store Page
							</CommandButton>
						</>
					)}
				</Button.Group>
				{modLoaders.map(
					(modLoader) =>
						modLoader.mods.length > 0 && (
							<Fragment key={modLoader.id}>
								<label>{modLoader.id} mods</label>
								<Button.Group orientation="vertical">
									{modLoader.mods
										.filter(
											(mod) =>
												mod.scriptingBackend === props.game.scriptingBackend,
										)
										.map((mod) =>
											props.game.installedMods.includes(mod.id) ? (
												<CommandButton
													leftSection={<IconTrash />}
													key={mod.name}
													onClick={async () => {
														await uninstallMod(props.game.id, mod.id);
														props.refreshGame(props.game.id);
													}}
												>
													Uninstall {mod.name}
												</CommandButton>
											) : (
												<CommandButton
													leftSection={<IconDownload />}
													key={mod.name}
													onClick={async () => {
														await installMod(
															modLoader.id,
															mod.id,
															props.game.id,
														);
														props.refreshGame(props.game.id);
													}}
												>
													Install {mod.name}
												</CommandButton>
											),
										)}
								</Button.Group>
							</Fragment>
						),
				)}
				<Stack gap="xs">
					<label>Debug Data</label>
					<CodeHighlight
						code={debugData}
						language="json"
					/>
				</Stack>
			</Stack>
		</Modal>
	);
}
