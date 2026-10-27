import { useEffect, useState } from "react";
import { api } from "./api";
import type { Accounts } from "./types";
import type { NameSource, Prefs } from "./prefs";
import { usePrefs } from "./prefs";

/** The sources that have a name right now, in the order "auto" tries them. */
export function nameOptions(accounts: Accounts | null, customName: string): { source: NameSource; name: string }[] {
  const out: { source: NameSource; name: string }[] = [];
  if (accounts?.steam) out.push({ source: "steam", name: accounts.steam });
  if (accounts?.epic) out.push({ source: "epic", name: accounts.epic });
  if (accounts?.xbox) out.push({ source: "xbox", name: accounts.xbox });
  if (accounts?.gog) out.push({ source: "gog", name: accounts.gog });
  if (customName.trim()) out.push({ source: "custom", name: customName.trim() });
  return out;
}

/**
 * The name the console shows. A chosen source that has no name (signed out
 * of Epic, say) falls back to the automatic choice rather than to nothing.
 */
export function pickName(accounts: Accounts | null, p: Pick<Prefs, "nameSource" | "customName">): { source: NameSource; name: string } | null {
  if (p.nameSource === "off") return null;
  const options = nameOptions(accounts, p.customName);
  const chosen = options.find((o) => o.source === p.nameSource);
  const auto = options.find((o) => o.source !== "custom");
  return chosen ?? auto ?? null;
}

/** "Koushik's", "James'": for headings like "Koushik's playtime". */
export function possessive(name: string): string {
  return /s$/i.test(name) ? `${name}’` : `${name}’s`;
}

/** Store accounts, kept current when a sign-in finishes elsewhere. */
export function useAccounts(): Accounts | null {
  const [accounts, setAccounts] = useState<Accounts | null>(null);
  useEffect(() => {
    const load = () => api.accounts().then(setAccounts);
    load();
    const off = api.onAccountsChanged(load);
    return () => void off.then((f) => f());
  }, []);
  return accounts;
}

export function useGamerTag(): string | null {
  const accounts = useAccounts();
  const p = usePrefs();
  return pickName(accounts, p)?.name ?? null;
}
