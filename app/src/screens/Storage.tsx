import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import { measureInstall, savedInfo, useSavedInfo } from "../lib/gameInfo";
import { formatBytes } from "../lib/format";
import { driveRoot, groupByDrive, idleHint, installPath, type DriveSpace, type DriveUse, type StoredGame } from "../lib/storage";
import type { Game } from "../lib/types";
import { Icon } from "../components/Icon";
import { openUninstall } from "../components/Uninstall";

/** Rows shown per drive before "Show all". */
const FIRST_ROWS = 10;

/**
 * Installed games and the drives they're on, from saved sizes (nothing is
 * measured here). Storage and the Storage block on Stats both read it.
 */
export function useDriveUse(version: number) {
  const [games, setGames] = useState<Game[] | null>(null);
  const [drives, setDrives] = useState<DriveSpace[] | null>(null);
  useSavedInfo();

  useEffect(() => {
    api.listGames({ install: "all", stores: [], search: null, hidden_only: false }).then(setGames);
  }, [version]);

  // Hidden games still take space, so they count here too.
  const [hidden, setHidden] = useState<Game[]>([]);
  useEffect(() => {
    api.listGames({ install: "all", stores: [], search: null, hidden_only: true }).then(setHidden, () => setHidden([]));
  }, [version]);

  const onDisk = useMemo(() => {
    const seen = new Set<number>();
    return [...(games ?? []), ...hidden].filter((g) => {
      if (seen.has(g.id) || !(g.installed || g.offline_drive)) return false;
      seen.add(g.id);
      return installPath(g) !== null;
    });
  }, [games, hidden]);

  // Network shares games are installed on, beside the local drives.
  const roots = useMemo(
    () => [...new Set(onDisk.map((g) => driveRoot(installPath(g)!)).filter((r): r is string => !!r?.startsWith("\\\\")))],
    [onDisk],
  );
  useEffect(() => {
    if (!games) return;
    api.storageDrives(roots).then(setDrives, () => setDrives([]));
  }, [games, roots.join("|")]);

  const stored: StoredGame[] = onDisk.map((g) => ({
    game: g,
    path: installPath(g)!,
    bytes: savedInfo(g.id)?.install?.bytes ?? null,
  }));
  const use = drives ? groupByDrive(stored, drives) : null;
  const refreshDrives = () => api.storageDrives(roots).then(setDrives, () => {});
  return { loaded: games !== null, onDisk, use, refreshDrives };
}

/**
 * What takes space, per drive: how full each drive is, how much of that is
 * games, and the games on it largest first, with how long since each was
 * played so the ones to uninstall stand out. Opened from Stats.
 */
export function Storage({
  version,
  onOpenGame,
  say,
}: {
  version: number;
  onOpenGame: (g: Game) => void;
  say: (text: string) => void;
}) {
  const { loaded, onDisk, use, refreshDrives } = useDriveUse(version);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const busy = useRef(false);

  // Sizes are measured one game at a time, off the UI thread: the ones never
  // measured when the page opens, or every installed game on "Measure again".
  const measure = async (all: boolean) => {
    if (busy.current) return;
    const todo = onDisk.filter((g) => g.installed && (all || savedInfo(g.id)?.install?.bytes == null));
    if (todo.length === 0) return;
    busy.current = true;
    setProgress({ done: 0, total: todo.length });
    for (let i = 0; i < todo.length; i++) {
      await measureInstall(todo[i].id, all).catch(() => null);
      setProgress({ done: i + 1, total: todo.length });
    }
    busy.current = false;
    setProgress(null);
    // Free space moves too when games update or install.
    void refreshDrives();
  };
  const autoMeasured = useRef(false);
  useEffect(() => {
    if (!loaded || autoMeasured.current) return;
    autoMeasured.current = true;
    void measure(false);
  }, [loaded]);

  if (!use) return <div className="scroll" />;

  const totalGames = use.reduce((n, d) => n + d.gameBytes, 0);
  const withGames = use.filter((d) => d.games.length > 0);

  return (
    <div className="scroll stats-scroll">
      <div className="stats storage">
        <h1>Storage</h1>
        <div className="storage-sub">
          <p>
            {onDisk.length === 0 ? (
              "No installed games yet."
            ) : (
              <>
                <b>{formatBytes(totalGames)}</b> of games across {withGames.length} {withGames.length === 1 ? "drive" : "drives"}
              </>
            )}
          </p>
          {progress ? (
            <span className="storage-progress" role="status">
              Measuring {Math.min(progress.done + 1, progress.total)} of {progress.total}…
            </span>
          ) : (
            onDisk.some((g) => g.installed) && (
              <button className="btn" data-nav onClick={() => void measure(true)} title="Sizes change when games update; measure every install folder again">
                <Icon name="refresh" size={15} />
                Measure again
              </button>
            )
          )}
        </div>

        {use.map((d) => (
          <Drive key={d.root} d={d} onOpenGame={onOpenGame} say={say} />
        ))}
      </div>
    </div>
  );
}

function Drive({ d, onOpenGame, say }: { d: DriveUse; onOpenGame: (g: Game) => void; say: (text: string) => void }) {
  const [all, setAll] = useState(false);
  // Folding away two or three rows saves nothing; only long lists fold.
  const folds = d.games.length > FIRST_ROWS + 3;
  const shown = all || !folds ? d.games : d.games.slice(0, FIRST_ROWS);
  const unmeasured = d.games.filter((g) => g.bytes === null).length;
  // Bars compare games on this drive with each other: against the whole
  // drive even a 100 GB game is a sliver.
  const largest = Math.max(1, ...d.games.map((g) => g.bytes ?? 0));
  const s = d.space;
  const used = s ? s.total - s.free : 0;
  const other = s ? Math.max(0, used - d.gameBytes) : 0;
  const pct = (n: number) => (s ? `${Math.min(100, (n / s.total) * 100)}%` : "0%");
  const name = d.label ? `${d.root} ${d.label}` : d.root;
  const empty = d.games.length === 0;

  const openFolder = (g: Game) =>
    api.openInstallFolder(g.id).catch((e) => say(`Couldn't open the folder of ${g.title}: ${e}`));

  return (
    <section className={`drive${empty ? " drive-empty" : ""}${s ? "" : " drive-offline"}`}>
      <header className="drive-head">
        <h2>
          <Icon name="drive" size={18} />
          {d.root}
          {d.label && <span>{d.label}</span>}
        </h2>
        <p>
          {s ? (
            <>
              <b>{formatBytes(s.free)}</b> free of {formatBytes(s.total)}
            </>
          ) : (
            "Not connected"
          )}
        </p>
      </header>

      {s && (
        <>
          <div
            className="drive-bar"
            role="img"
            aria-label={`${name}: games ${formatBytes(d.gameBytes)}, everything else ${formatBytes(other)}, ${formatBytes(s.free)} free`}
          >
            <span className="seg-games" style={{ width: pct(d.gameBytes) }} />
            <span className="seg-other" style={{ width: pct(other) }} />
          </div>
          {!empty && (
            <p className="drive-legend">
              <span>
                <i className="seg-games" /> Games {formatBytes(d.gameBytes)}
                {unmeasured > 0 && <small> ({unmeasured} not measured yet)</small>}
              </span>
              <span>
                <i className="seg-other" /> Everything else {formatBytes(other)}
              </span>
            </p>
          )}
        </>
      )}
      {!s && <p className="drive-note">These sizes are from when the drive was last connected.</p>}

      {empty ? (
        <p className="drive-note">No games installed here.</p>
      ) : (
        <ul className="drive-games">
          {shown.map(({ game, bytes }) => {
            const hint = idleHint(game);
            return (
              <li key={game.id}>
                <button className="drive-game" data-nav onClick={() => onOpenGame(game)} title={`Show ${game.title} in the library`}>
                  {game.cover ? <img src={game.cover} alt="" loading="lazy" /> : <span className="drive-art" />}
                  <span className="drive-name">
                    <b>{game.title}</b>
                    {hint && <small>{hint}</small>}
                  </span>
                  <span className="drive-size">{bytes === null ? "Not measured" : formatBytes(bytes)}</span>
                  <span className="drive-share" aria-hidden>
                    {bytes !== null && <span style={{ width: `${Math.max(2, (bytes / largest) * 100)}%` }} />}
                  </span>
                </button>
                {/* Nothing to open or remove on a missing drive; the space stays so columns line up. */}
                {!s && <span className="drive-gap" aria-hidden />}
                {s && (
                  <span className="drive-actions">
                    <button className="icon-btn" data-nav title="Open install folder" aria-label={`Open the install folder of ${game.title}`} onClick={() => void openFolder(game)}>
                      <Icon name="folder" size={16} />
                    </button>
                    <button className="icon-btn" data-nav title="Uninstall…" aria-label={`Uninstall ${game.title}`} onClick={() => openUninstall(game)}>
                      <Icon name="trash" size={16} />
                    </button>
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {folds && (
        <button className="link drive-more" data-nav onClick={() => setAll(!all)}>
          {all ? "Show fewer" : `Show all ${d.games.length} games`}
        </button>
      )}
    </section>
  );
}
