import { Button, Flex, Input, Stack, Text } from "@mantine/core";
import { MdRefresh } from "react-icons/md";
import { OwnedGameRow } from "./owned-game-row";
import { useOwnedUnityGames } from "@hooks/use-backend-data";
import { OwnedUnityGame } from "@api/bindings";
import { useState } from "react";
import { includesOneOf } from "../../util/filter";
import { OwnedGameModal } from "./owned-game-modal";
import { TableHeader } from "@components/table/table-head";
import { useFilteredList } from "@hooks/use-filtered-list";
import { FilterMenu } from "@components/filter-menu";
import { VirtualizedTable } from "@components/table/virtualized-table";
import { SwitchButton } from "@components/switch-button";

const tableHeaders: TableHeader<OwnedUnityGame, keyof OwnedUnityGame>[] = [
  { id: "name", label: "Game", width: undefined },
  { id: "osList", label: "Linux?", width: 100, center: true },
  { id: "installed", label: "Installed?", width: 100, center: true },
];

type Filter = {
  text: string;
  hideInstalled: boolean;
  linuxOnly: boolean;
};

const defaultFilter: Filter = {
  text: "",
  hideInstalled: false,
  linuxOnly: false,
};

const filterGame = (game: OwnedUnityGame, filter: Filter) =>
  includesOneOf(filter.text, [game.name, game.id.toString()]) &&
  (!filter.linuxOnly || game.osList.includes("Linux")) &&
  (!filter.hideInstalled || !game.installed);

export function OwnedGamesPage() {
  const [ownedGames, isLoading, refreshOwnedGames] = useOwnedUnityGames();
  const [selectedGame, setSelectedGame] = useState<OwnedUnityGame>();

  const [filteredGames, sort, setSort, filter, setFilter] = useFilteredList(
    tableHeaders,
    ownedGames,
    filterGame,
    defaultFilter
  );

  return (
    <Stack h="100%">
      {selectedGame ? (
        <OwnedGameModal
          selectedGame={selectedGame}
          onClose={() => setSelectedGame(undefined)}
        />
      ) : null}
      <Flex gap="md">
        <Input
          placeholder="Find..."
          onChange={(event) => setFilter({ text: event.target.value })}
          style={{ flex: 1 }}
        />
        <FilterMenu>
          <Stack>
            <SwitchButton
              value={filter.hideInstalled}
              onChange={(value) => setFilter({ hideInstalled: value })}
            >
              Hide installed games
            </SwitchButton>
            <SwitchButton
              value={filter.linuxOnly}
              onChange={(value) => setFilter({ linuxOnly: value })}
            >
              Hide games without native Linux support
            </SwitchButton>
          </Stack>
        </FilterMenu>
        <Button
          leftSection={<MdRefresh />}
          loading={isLoading}
          onClick={refreshOwnedGames}
          style={{ flex: 1, maxWidth: 200 }}
        >
          {isLoading ? "Finding owned games..." : "Refresh"}
        </Button>
      </Flex>
      <Text>
        These are the Steam games you own (maybe?) that use the Unity engine
        (maybe??). {ownedGames.length} owned games.
      </Text>
      <VirtualizedTable
        data={filteredGames}
        itemContent={OwnedGameRow}
        headerItems={tableHeaders}
        sort={sort}
        onChangeSort={setSort}
        onClickItem={setSelectedGame}
      />
    </Stack>
  );
}
