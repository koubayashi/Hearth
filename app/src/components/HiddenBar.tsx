import { Icon } from "./Icon";

/** Heads the hidden view, which opens from Settings and has no rail item of
 *  its own: says where you are, and gives the way back. */
export function HiddenBar({ count, onBack }: { count: number | null; onBack: () => void }) {
  return (
    <div className="hidden-bar" role="note">
      <span className="hidden-bar-icon" aria-hidden>
        <Icon name="hidden" size={18} />
      </span>
      <div className="text">
        <b>
          Hidden games
          {count !== null && <small>{count}</small>}
        </b>
        <span>Kept out of the library. Open a game's menu to bring it back.</span>
      </div>
      <button className="btn" data-nav onClick={onBack}>
        <Icon name="chevronLeft" size={15} />
        Back to library
      </button>
    </div>
  );
}
