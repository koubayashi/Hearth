//! Series, franchises, developers and publishers from Wikidata, for the
//! library's "browse by" shelves. No key needed. Games are found by their
//! Steam app id (exact), else by their exact English title; companies and
//! series are linked to their parents (Ubisoft Montreal to Ubisoft, a series
//! to its franchise) so a filter on the parent includes them.

use std::collections::HashMap;

use anyhow::Context;
use serde::Serialize;

const SPARQL: &str = "https://query.wikidata.org/sparql";
const API: &str = "https://www.wikidata.org/w/api.php";
/// Between searches, which Wikidata rate-limits harder than queries.
const SEARCH_GAP: std::time::Duration = std::time::Duration::from_millis(1500);
/// After a "too many requests", before asking once more.
const RETRY_AFTER: std::time::Duration = std::time::Duration::from_secs(10);

/// What a link says about a game. A franchise is kept apart from a series
/// so the shelves can put a game's series inside its franchise (Hogwarts
/// Legacy inside Wizarding World) even where Wikidata doesn't link the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Series,
    Franchise,
    Developer,
    Publisher,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Series => "series",
            Role::Franchise => "franchise",
            Role::Developer => "developer",
            Role::Publisher => "publisher",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub role: Role,
    pub id: String,
    pub label: String,
}

/// One Wikidata item that matched a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub links: Vec<Link>,
}

/// One "is part of" step: a studio's current parent company, or a series'
/// parent series or franchise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parent {
    pub child: String,
    pub id: String,
    pub label: String,
}

/// The links a game gets, by role. Publishers leave out ports for other
/// systems and regional releases (Feral's Mac version, a Russian
/// distributor): every PC copy would otherwise list them.
const LINKS: &str = r#"
  OPTIONAL {
    { ?game wdt:P179 ?x BIND("series" AS ?role) } UNION
    { ?game wdt:P8345 ?x BIND("franchise" AS ?role) } UNION
    { ?game wdt:P178 ?x BIND("developer" AS ?role) } UNION
    { ?game p:P123 ?st . ?st ps:P123 ?x . ?st wikibase:rank ?rank .
      FILTER(?rank != wikibase:DeprecatedRank)
      FILTER NOT EXISTS { ?st pq:P400 [] . FILTER NOT EXISTS { ?st pq:P400 wd:Q1406 } }
      FILTER NOT EXISTS { ?st pq:P291 ?place . FILTER(?place NOT IN (wd:Q30, wd:Q13780930, wd:Q46, wd:Q49, wd:Q145)) }
      FILTER NOT EXISTS { ?st pq:P2541 [] }
      BIND("publisher" AS ?role) }
  }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en,mul". }
"#;

/// A store's edition is often an item of its own with none of the links
/// ("Grand Theft Auto V Enhanced", "Control: Ultimate Edition"): the game
/// it's an edition of, or the game it's based on, lends it its own. A
/// compilation ("HITMAN World of Assassination") takes them from its parts.
const ORIGINAL: &str = "OPTIONAL {
    { ?game wdt:P629|wdt:P144 ?base . ?base wdt:P31 wd:Q7889 . }
    UNION { ?game wdt:P31 wd:Q16070115 ; wdt:P527 ?base . }
  }";

/// For each key, the items found and the originals each borrows links from.
type Found = HashMap<String, Vec<(String, Vec<String>)>>;

pub struct Wikidata {
    http: reqwest::Client,
}

impl Default for Wikidata {
    fn default() -> Self {
        Self {
            // Wikidata asks every client to say who it is.
            http: reqwest::Client::builder()
                .user_agent("Hearth/0.1 (personal game library)")
                // Wikidata stops a query at 60s; past that, try again next sync.
                .timeout(std::time::Duration::from_secs(70))
                .build()
                .expect("http client"),
        }
    }
}

impl Wikidata {
    /// Items for each Steam app id. One id can have several (a game and its
    /// online mode): `pick` chooses.
    pub async fn by_steam(&self, apps: &[u64]) -> anyhow::Result<HashMap<u64, Vec<Item>>> {
        if apps.is_empty() {
            return Ok(HashMap::new());
        }
        let keys: Vec<String> = apps.iter().map(|a| format!("\"{a}\"")).collect();
        let found = self.resolve(&format!("VALUES ?key {{ {} }} ?game wdt:P1733 ?key .", keys.join(" "))).await?;
        let items = self.assemble(found).await?;
        Ok(items.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect())
    }

    /// Video games named exactly one of these, by title. Many names are now
    /// kept only as the "mul" (every language) label, so both are asked for.
    pub async fn by_title(&self, titles: &[String]) -> anyhow::Result<HashMap<String, Vec<Item>>> {
        if titles.is_empty() {
            return Ok(HashMap::new());
        }
        let keys: Vec<String> = titles.iter().flat_map(|t| [format!("{}@en", literal(t)), format!("{}@mul", literal(t))]).collect();
        let found = self.resolve(&format!("VALUES ?key {{ {} }} ?game rdfs:label ?key ; wdt:P31 wd:Q7889 .", keys.join(" "))).await?;
        let mut items = self.assemble(found).await?;
        // Answers come back under the cleaned title ("Hades", not "Hades™").
        Ok(titles.iter().filter_map(|t| Some((t.clone(), items.remove(clean(t).as_str())?))).collect())
    }

    /// For titles that named nothing exactly ("HITMAN", "Microsoft Flight
    /// Simulator (2020)", "Trackmania Starter Access"): Wikidata's search
    /// among video games, keeping only items whose name is the title once
    /// edition words and brackets are set aside. One search per title, so
    /// only for the few left over.
    ///
    /// Also returns the titles it couldn't search (Wikidata kept saying
    /// "too many requests"): those are for next time, not "not found".
    pub async fn by_search(&self, titles: &[String]) -> anyhow::Result<(HashMap<String, Vec<Item>>, Vec<String>)> {
        let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
        let mut unsearched = Vec::new();
        for (i, t) in titles.iter().enumerate() {
            // The search API turns away quick bursts (429): a short pause between titles.
            if i > 0 {
                tokio::time::sleep(SEARCH_GAP).await;
            }
            match self.search(t).await {
                Ok(ids) => candidates.push((t.clone(), ids)),
                // Still refused after a wait: the rest would be too.
                Err(_) => {
                    unsearched.extend(titles[i..].iter().cloned());
                    break;
                }
            }
        }
        let mut all: Vec<String> = candidates.iter().flat_map(|(_, ids)| ids.iter().map(|i| format!("wd:{i}"))).collect();
        all.sort();
        all.dedup();
        if all.is_empty() {
            return Ok((HashMap::new(), unsearched));
        }
        // Name lookup returns films and franchises too: video games only.
        let found = self
            .resolve(&format!(
                "VALUES ?game {{ {} }} ?game wdt:P31 wd:Q7889 . BIND(STRAFTER(STR(?game), \"entity/\") AS ?key)",
                all.join(" ")
            ))
            .await?;
        let items = self.assemble(found).await?;
        let found = candidates
            .into_iter()
            .filter_map(|(title, ids)| {
                let want = core(&title);
                let matched: Vec<Item> = ids
                    .iter()
                    .filter_map(|i| items.get(i))
                    .flatten()
                    .filter(|item| core(&item.label) == want)
                    .cloned()
                    .collect();
                (!matched.is_empty()).then_some((title, matched))
            })
            .collect();
        Ok((found, unsearched))
    }

    /// Candidate items for one title. Name lookup finds "Hitman" itself;
    /// full-text search, which ranks sequels above it, finds names worded
    /// differently.
    async fn search(&self, title: &str) -> anyhow::Result<Vec<String>> {
        let words = core_words(title).join(" ");
        let mut ids = self
            .search_ids(&[("action", "wbsearchentities"), ("search", &words), ("language", "en"), ("type", "item"), ("limit", "10")], "/search", "id")
            .await?;
        tokio::time::sleep(SEARCH_GAP).await;
        let text = format!("{words} haswbstatement:P31=Q7889");
        let more = self.search_ids(&[("action", "query"), ("list", "search"), ("srsearch", &text), ("srlimit", "5")], "/query/search", "title").await?;
        ids.extend(more.into_iter().filter(|i| !ids.contains(i)).collect::<Vec<_>>());
        Ok(ids)
    }

    /// Item ids from one of Wikidata's search APIs: the array at `list`,
    /// each hit's id under `field`. Turned away for asking too often (429),
    /// it waits and asks once more.
    async fn search_ids(&self, params: &[(&str, &str)], list: &str, field: &str) -> anyhow::Result<Vec<String>> {
        let ask = || self.http.get(API).query(params).query(&[("format", "json")]).send();
        let mut res = ask().await?;
        if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            tokio::time::sleep(RETRY_AFTER).await;
            res = ask().await?;
        }
        let body: serde_json::Value = res.error_for_status()?.json().await.context("searching Wikidata")?;
        Ok(body
            .pointer(list)
            .and_then(|s| s.as_array())
            .map(|hits| hits.iter().filter_map(|h| h.get(field)?.as_str()).filter(|i| is_qid(i)).map(str::to_string).collect())
            .unwrap_or_default())
    }

    /// Step one: which items a query's keys name, and what each is an edition of.
    /// Kept apart from the links, which made a single query time out.
    async fn resolve(&self, head: &str) -> anyhow::Result<Found> {
        let rows = self.select(&format!("SELECT ?key ?game ?base WHERE {{ {head} {ORIGINAL} }}")).await?;
        let mut found: Found = HashMap::new();
        for r in rows {
            let (Some(key), Some(game)) = (r.get("key"), r.get("game").and_then(|g| entity(g))) else { continue };
            let base = r.get("base").and_then(|b| entity(b));
            let list = found.entry(key.clone()).or_default();
            let at = match list.iter().position(|(g, _)| *g == game) {
                Some(at) => at,
                None => {
                    list.push((game, Vec::new()));
                    list.len() - 1
                }
            };
            if let Some(b) = base.filter(|b| !list[at].1.contains(b)) {
                list[at].1.push(b);
            }
        }
        Ok(found)
    }

    /// Step two: the links of every item found and of the originals, each
    /// edition taking its original's links on top of its own.
    async fn assemble(&self, found: Found) -> anyhow::Result<HashMap<String, Vec<Item>>> {
        let mut ids: Vec<String> = found.values().flatten().flat_map(|(g, b)| std::iter::once(g.clone()).chain(b.iter().cloned())).collect();
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let values: Vec<String> = ids.iter().map(|i| format!("wd:{i}")).collect();
        let query = format!(
            "SELECT ?key ?game ?gameLabel ?role ?x ?xLabel WHERE {{ VALUES ?game {{ {} }} BIND(STRAFTER(STR(?game), \"entity/\") AS ?key) {LINKS} }}",
            values.join(" ")
        );
        let by_id: HashMap<String, Item> = group(&self.select(&query).await?)
            .into_iter()
            .filter_map(|(k, mut v)| Some((k, v.pop()?)))
            .collect();
        Ok(found
            .into_iter()
            .map(|(key, games)| {
                let items = games
                    .into_iter()
                    .filter_map(|(game, bases)| {
                        let mut item = by_id.get(&game)?.clone();
                        for link in bases.iter().filter_map(|b| by_id.get(b)).flat_map(|b| b.links.clone()) {
                            if !item.links.contains(&link) {
                                item.links.push(link);
                            }
                        }
                        Some(item)
                    })
                    .collect();
                (key, items)
            })
            .collect())
    }

    /// The parents of these companies and series. Only a company's current
    /// owners: a parent with an end date (a studio sold years ago) is left out.
    pub async fn parents(&self, ids: &[String]) -> anyhow::Result<Vec<Parent>> {
        let ids: Vec<String> = ids.iter().filter(|i| is_qid(i)).map(|i| format!("wd:{i}")).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let query = format!(
            "SELECT ?x ?p ?pLabel WHERE {{ VALUES ?x {{ {} }}
               {{ ?x p:P749 ?st . ?st ps:P749 ?p . ?st wikibase:rank ?r .
                  FILTER(?r != wikibase:DeprecatedRank) FILTER NOT EXISTS {{ ?st pq:P582 [] }} }}
               UNION {{ ?x wdt:P179 ?p }} UNION {{ ?x wdt:P8345 ?p }}
               SERVICE wikibase:label {{ bd:serviceParam wikibase:language \"en,mul\". }} }}",
            ids.join(" ")
        );
        let rows = self.select(&query).await?;
        let mut seen = std::collections::HashSet::new();
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some(Parent { child: entity(r.get("x")?)?, id: entity(r.get("p")?)?, label: r.get("pLabel")?.clone() })
            })
            // A parent with no English name would show as its id.
            .filter(|p| p.child != p.id && !is_qid(&p.label))
            .filter(|p| seen.insert((p.child.clone(), p.id.clone())))
            .collect())
    }

    async fn select(&self, query: &str) -> anyhow::Result<Vec<HashMap<String, String>>> {
        let body: serde_json::Value = self
            .http
            .post(SPARQL)
            .header("Accept", "application/sparql-results+json")
            .form(&[("query", query)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("reading Wikidata")?;
        Ok(rows(&body))
    }
}

/// The best item for a game: the one whose name is the game's title, else
/// the one that names it, else the one with the most links. Names are
/// compared without edition words ("Control Ultimate Edition" is "Control").
pub fn pick<'a>(items: &'a [Item], title: &str) -> Option<&'a Item> {
    let want = core(title);
    let score = |i: &Item| {
        let have = core(&i.label);
        let name = if have == want {
            2
        } else if !have.is_empty() && (want.contains(&have) || have.contains(&want)) {
            1
        } else {
            0
        };
        (name, i.links.len())
    };
    items.iter().max_by_key(|i| score(i))
}

/// Words that name a store's edition of a game, not the game. Matched as
/// whole words at the end of a title, repeatedly ("Premier Edition").
const EDITION_TAILS: &[&[&str]] = &[
    &["edition"],
    &["ultimate"],
    &["complete"],
    &["definitive"],
    &["deluxe"],
    &["gold"],
    &["premium"],
    &["premier"],
    &["goty"],
    &["game", "of", "the", "year"],
    &["enhanced"],
    &["directors", "cut"],
    &["starter", "access"],
    &["demo"],
];

/// A title's words, lowercase, without trademark signs, anything in
/// brackets ("(2020)") or edition words at the end. Never empties a title.
fn core_words(s: &str) -> Vec<String> {
    let mut t = s.to_lowercase().replace(['™', '®'], "").replace(['\'', '’'], "");
    while let (Some(a), Some(b)) = (t.find('('), t.find(')')) {
        if a >= b {
            break;
        }
        t.replace_range(a..=b, " ");
    }
    let mut words: Vec<String> = t.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_string).collect();
    'strip: loop {
        for tail in EDITION_TAILS {
            if words.len() > tail.len() && words[words.len() - tail.len()..].iter().zip(tail.iter()).all(|(w, t)| w == t) {
                words.truncate(words.len() - tail.len());
                continue 'strip;
            }
        }
        break;
    }
    words
}

/// A title reduced for comparing: "Call of Duty - Modern Warfare Remastered"
/// and "Call of Duty: Modern Warfare Remastered" are the same.
fn core(s: &str) -> String {
    core_words(s).concat()
}

/// A title as Wikidata would name it: without trademark signs.
fn clean(s: &str) -> String {
    s.replace(['™', '®'], "").trim().to_string()
}

/// A SPARQL string literal.
fn literal(s: &str) -> String {
    format!("\"{}\"", clean(s).replace('\\', "\\\\").replace('"', "\\\""))
}

fn is_qid(s: &str) -> bool {
    s.len() > 1 && s.starts_with('Q') && s[1..].bytes().all(|b| b.is_ascii_digit())
}

/// "http://www.wikidata.org/entity/Q42" → "Q42".
fn entity(uri: &str) -> Option<String> {
    let id = uri.rsplit('/').next()?;
    is_qid(id).then(|| id.to_string())
}

/// SPARQL JSON results as rows of variable → value.
fn rows(body: &serde_json::Value) -> Vec<HashMap<String, String>> {
    let Some(bindings) = body.pointer("/results/bindings").and_then(|b| b.as_array()) else { return Vec::new() };
    bindings
        .iter()
        .filter_map(|b| b.as_object())
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.get("value")?.as_str()?.to_string())))
                .collect()
        })
        .collect()
}

/// Rows of (key, game, role, link) gathered into items per key.
fn group(rows: &[HashMap<String, String>]) -> HashMap<String, Vec<Item>> {
    let mut out: HashMap<String, Vec<Item>> = HashMap::new();
    for r in rows {
        let (Some(key), Some(game)) = (r.get("key"), r.get("game").and_then(|g| entity(g))) else { continue };
        let items = out.entry(key.clone()).or_default();
        let at = match items.iter().position(|i| i.id == game) {
            Some(at) => at,
            None => {
                let label = r.get("gameLabel").cloned().unwrap_or_default();
                items.push(Item { id: game, label, links: Vec::new() });
                items.len() - 1
            }
        };
        let role = match r.get("role").map(String::as_str) {
            Some("series") => Role::Series,
            Some("franchise") => Role::Franchise,
            Some("developer") => Role::Developer,
            Some("publisher") => Role::Publisher,
            _ => continue,
        };
        let (Some(id), Some(label)) = (r.get("x").and_then(|x| entity(x)), r.get("xLabel")) else { continue };
        // A link with no name in English (the label service falls back to the id) is no use on a shelf.
        if is_qid(label) {
            continue;
        }
        let link = Link { role, id, label: label.clone() };
        if !items[at].links.contains(&link) {
            items[at].links.push(link);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
    const E: &str = "http://www.wikidata.org/entity/";

    #[test]
    fn groups_rows_into_items_per_key() {
        let rows = [
            row(&[("key", "271590"), ("game", &format!("{E}Q17452")), ("gameLabel", "Grand Theft Auto V"), ("role", "series"), ("x", &format!("{E}Q132730")), ("xLabel", "Grand Theft Auto")]),
            row(&[("key", "271590"), ("game", &format!("{E}Q17452")), ("gameLabel", "Grand Theft Auto V"), ("role", "developer"), ("x", &format!("{E}Q94893")), ("xLabel", "Rockstar North")]),
            row(&[("key", "271590"), ("game", &format!("{E}Q14594724")), ("gameLabel", "Grand Theft Auto Online")]),
            // No English name: dropped rather than shown as "Q959329".
            row(&[("key", "271590"), ("game", &format!("{E}Q17452")), ("gameLabel", "Grand Theft Auto V"), ("role", "developer"), ("x", &format!("{E}Q959329")), ("xLabel", "Q959329")]),
        ];
        let g = group(&rows);
        let items = &g["271590"];
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].links.len(), 2);
        assert_eq!(pick(items, "Grand Theft Auto V").unwrap().id, "Q17452");
    }

    #[test]
    fn picks_by_title_then_by_links() {
        let item = |id: &str, label: &str, links: usize| Item {
            id: id.into(),
            label: label.into(),
            links: (0..links).map(|i| Link { role: Role::Developer, id: format!("Q{i}"), label: "x".into() }).collect(),
        };
        let items = [item("Q1", "Grand Theft Auto Online", 5), item("Q2", "Grand Theft Auto V", 1)];
        assert_eq!(pick(&items, "Grand Theft Auto V™").unwrap().id, "Q2");
        let items = [item("Q1", "Something", 1), item("Q2", "Else", 3)];
        assert_eq!(pick(&items, "Unrelated").unwrap().id, "Q2");
    }

    /// Asks the real Wikidata: `cargo test -p hub-providers wikidata -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_series_and_studios() {
        let wd = Wikidata::default();
        let found = wd.by_steam(&[204100, 2488620, 2591280]).await.unwrap();
        let series = |app: u64, title: &str| -> Vec<String> {
            let item = pick(&found[&app], title).unwrap();
            item.links.iter().filter(|l| l.role == Role::Series).map(|l| l.label.clone()).collect()
        };
        assert!(series(204100, "Max Payne 3").contains(&"Max Payne".to_string()), "same series, other studio");
        assert!(series(2488620, "F1 24").contains(&"F1".to_string()));
        assert!(!series(2591280, "F1 Manager 2024").contains(&"F1".to_string()), "a different series");

        let titled = wd.by_title(&["Hades™".to_string()]).await.unwrap();
        assert!(titled.contains_key("Hades™"), "found by its plain name");

        let parents = wd.parents(&["Q903621".to_string()]).await.unwrap();
        assert!(parents.iter().any(|p| p.label == "Ubisoft"), "{parents:?}");
    }

    #[test]
    fn compares_titles_without_edition_words() {
        assert_eq!(core("Control Ultimate Edition"), "control");
        assert_eq!(core("DEATH STRANDING DIRECTOR'S CUT"), "deathstranding");
        assert_eq!(core("Microsoft Flight Simulator (2020)"), "microsoftflightsimulator");
        assert_eq!(core("Lifeless Planet: Premier Edition"), "lifelessplanet");
        assert_eq!(core("Trackmania Starter Access"), "trackmania");
        assert_eq!(core("Call of Duty - Modern Warfare Remastered"), core("Call of Duty: Modern Warfare Remastered"));
        // A title that is only an edition word keeps it.
        assert_eq!(core("Gold"), "gold");
    }

    /// Your library's misses, against the real Wikidata:
    /// `cargo test -p hub-providers wikidata -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_editions_and_loose_titles() {
        let wd = Wikidata::default();
        let found = wd.by_steam(&[3240220, 870780]).await.unwrap();
        let series = |items: &[Item], title: &str| -> Vec<String> {
            pick(items, title).unwrap().links.iter().filter(|l| l.role == Role::Series).map(|l| l.label.clone()).collect()
        };
        // Steam's id for the Enhanced edition: the series comes from GTA V itself.
        assert!(series(&found[&3240220], "Grand Theft Auto V").contains(&"Grand Theft Auto".to_string()));
        let control = pick(&found[&870780], "Control Ultimate Edition").unwrap();
        assert!(control.links.iter().any(|l| l.role == Role::Developer && l.label.contains("Remedy")), "{control:?}");

        // A compilation: its developer comes from the games it holds.
        let woa = wd.by_steam(&[1659040]).await.unwrap();
        let hitman = pick(&woa[&1659040], "HITMAN World of Assassination").unwrap();
        assert!(hitman.links.iter().any(|l| l.role == Role::Developer && l.label == "IO Interactive"), "{hitman:?}");

        let titles: Vec<String> = ["HITMAN", "VALORANT", "Microsoft Flight Simulator (2020)", "Call of Duty - Modern Warfare Remastered"]
            .map(String::from)
            .to_vec();
        let (searched, unsearched) = wd.by_search(&titles).await.unwrap();
        assert!(unsearched.is_empty(), "rate-limited: {unsearched:?}");
        for t in &titles {
            assert!(searched.contains_key(t), "{t} not found: {:?}", searched.keys());
        }
    }

    #[test]
    fn quotes_titles_safely() {
        assert_eq!(literal("Tom Clancy's \"Rainbow\" Six® \\"), "\"Tom Clancy's \\\"Rainbow\\\" Six \\\\\"");
    }
}
