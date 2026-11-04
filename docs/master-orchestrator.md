# MASTER-ORK — Pääorkestroija (Koeta Studio 1.0)

Tila: **running** (1.0-integraatio 2026-11-02) · Agentti: `.claude/agents/kilsat-master-orchestrator.md`

## Tavoite

Kaikki keskeneräinen työ yhteen ketjuun:

**haara → PR → CI vihreä → katselmus → `main` → `kilsatstudio1.0` → release-CI → julkaisukelpoinen `Koeta Studio.app`**

Pääorkestroija on ylin koordinoija. Se ohjaa:

- `kilsat-orchestrator` (arkkitehtuuri, A01–A10, yhteiset sopimukset)
- `kilsat-pro-orchestrator` (PRO-sarja, P01–P08)
- yksittäisiä A- ja P-agentteja korjauksiin omilla haaroillaan

## Lähtötiedot

- [AGENTS.md](../AGENTS.md), [release-1.0.md](release-1.0.md),
  [master-progress.md](master-progress.md)
- [agents/00-orkestroija.md](agents/00-orkestroija.md),
  [pro-studio/00-pro-orkestroija.md](pro-studio/00-pro-orkestroija.md)
- `.github/workflows/` — erityisesti `macos-release-app.yml` (ajetaan `main`ille,
  `kilsatstudio1.0`:lle ja `v*`-tageille) ja `macos-release.yml` (Developer ID +
  notarisointi)
- [mac-production-release.md](mac-production-release.md),
  [workstreams/mac-device-acceptance.md](workstreams/mac-device-acceptance.md)

## Sallittu muutosalue

- `docs/master-orchestrator.md`, `docs/master-progress.md` (yhdistämisen jälkeen),
  `docs/release-1.0.md` (tila ja avoimet asiat)
- Yhdistämiskonfliktien ratkaisu PR-haaroissa, kun ratkaisu on mekaaninen.
  Sisällölliset konfliktit palautetaan omistavalle agentille.
- `.github/workflows/**` vain CI:n korjaamiseen, ei uusia ominaisuuksia; muutos omana PR:nä.

## Työnkulku

### 1. Tilannekuva

```sh
git fetch --prune origin
git branch -r --no-merged origin/main
gh pr list --state open --json number,headRefName,title,mergeable,statusCheckRollup
git merge-base --is-ancestor origin/kilsatstudio1.0 origin/main && echo ff-ok
```

Täytä alla oleva haararekisteri: jokaiselle haaralle PR (numero tai puuttuu),
erotus mainiin (`git log --oneline origin/main..origin/<haara>`), CI-tila ja luokka:

- **ship** — kuuluu 1.0:aan, vie läpi
- **stale** — sisältö jo mainissa (tyhjä tai kirsikkapoimittu erotus); ehdota sulkemista
- **defer** — keskeneräinen tai 1.0:n ulkopuolinen; jätä koskematta
- **conflict** — vaatii omistavan agentin rebasen

Paikalliset haarat, joita ei ole pushattu, ja jäljittämättömät tiedostot kirjataan
käyttäjälle; niitä ei commitoida eikä poisteta ilman lupaa.

### 2. Suunnitelma ja lupa

Esitä käyttäjälle yksi taulukko: yhdistysjärjestys (riippuvuudet ja kuumat tiedostot
ensin, dokumentaatio-PR:t viimeisenä), stale-haarat ja defer-haarat. Pyydä
yhdistämislupa joko PR kohtaisesti tai yleislupana tälle listalle.

### 3. PR:t puuttuville haaroille

`ship`-haaralle ilman PR:ää: `gh pr create --base main --head <haara>` kuvauksella
(tehty, ajetut tarkistukset, avoimet asiat, attribuutiorivi). Tarkista, että PR
oikeasti syntyi (`gh pr view`).

### 4. Katselmus ja CI PR kerrallaan

- Tarkista muutosalue, uudet riippuvuudet (lisenssi, `Cargo.lock`), Storytold-rajaus
  ja ettei suuria malliassetteja ole Gitissä.
- Aja paikallisesti PR-haaralla työtilan juuresta:
  ```sh
  cargo fmt --all --check
  cargo check --workspace
  cargo clippy --workspace --all-targets
  cargo test --workspace
  ```
  ja sidecarin muuttuessa
  `python3 -m unittest discover -s services/kokoro-runtime/tests -v`.
- `gh pr checks <nro> --watch`. Punainen → syy omistavalle agentille, älä ohita.
- Konflikti tai vanhentunut pohja → omistava agentti rebasettaa uuden `main`in päälle.

### 5. Yhdistäminen

Vain vihreä CI + lupa: `gh pr merge <nro> --merge`. Ei squashia, ei force-pushia,
ei haarojen poistoa ilman lupaa. Odota `main`in CI vihreäksi ennen seuraavaa.

Jokaisen yhdistämisen jälkeen: päivitä `docs/master-progress.md` ja yhdistetyn PR:n
kuvaus (tehty, oikeasti ajetut tarkistukset, avoimet asiat) sekä rekisteri alla.
Kirjaukset omassa `docs/…`-PR:ssä, joka myös menee CI:n kautta.

### 6. `kilsatstudio1.0`

Kun kaikki `ship`-PR:t ovat `main`issa ja `main`in CI on vihreä:

```sh
git fetch origin
git merge-base --is-ancestor origin/kilsatstudio1.0 origin/main   # pakko onnistua
git push origin origin/main:refs/heads/kilsatstudio1.0             # ei --force
```

Jos fast-forward ei onnistu, pysähdy ja kysy. Seuraa `macos-release-app.yml`-ajoa
(`gh run watch`) ja kirjaa ajon URL ja ZIP-artefaktin nimi.

### 7. Tuotantovalmius

Merkitse kukin kohta vain näytön perusteella (ajon URL, komento ja tulos):

- [x] `main`in kaikki työnkulut vihreinä samalla commitilla — `1f18c48`, 7/7 (2026-11-02)
- [x] `macos-release-app.yml` vihreä `kilsatstudio1.0`:lla; ZIP-artefakti olemassa — ajo 41491467183, `1f18c48`
- [x] `package_audit.py` läpi: vain järjestelmäkirjastot, ei malliassetteja — release app -ajo 41480970816 (main)
- [x] Käynnistystesti CI:ssä (binääri käynnistyy ja raportoi version) — sama ajo
- [x] Versio `Info.plist`issa ja Cargo-paketeissa yhtenäinen (1.0.0) — sovellus, työtila ja kaikki cratet (motion-audio, motion-tts ja motion-lipsync siirretty `version.workspace = true`, 2026-11-04)
- [x] Kokoro puuttuu → selkeä virhe ja tuodun äänen polku toimii (testi) — `crates/motion-voice/tests/kokoro_missing.rs` oikealla `LocalSpeechProvider`illa: virhe nimeää puuttuvan tiedoston ja tuodun WAV:n vaihtoehdon, tuotu ääni valmistuu samalla palvelulla (2026-11-04)
- [x] Rust-riippuvuuksien lisenssiraportti ajan tasalla ja CI-tarkistettu — `scripts/third_party_licenses.py --check` (Audio foundation, macOS), `THIRD-PARTY-LICENSES.txt` Mac- ja iPad-paketissa, `docs/third-party-license-report.md`: 265 cratea, 0 vaatii päätöstä, 2 ilman tekstiä (block 0.1.6, dispatch 0.2.0) (2026-11-04)
- [ ] Puhepaketin lisenssit: piper-tts/eSpeak NG (GPL-3.0) lähdevelvoitteet, Kokoro- ja harri-äänten jakeluoikeudet, Python-riippuvuudet — **vaatii käyttäjän päätöksen**
- [ ] Developer ID -allekirjoitus ja notarisointi — **vaatii käyttäjän tunnukset**;
      ilman niitä paketti on ad-hoc-allekirjoitettu, ja se raportoidaan
- [ ] `v1.0.0`-tagi ja GitHub-julkaisu — **vain käyttäjän erillisellä luvalla**

Erillään (ei toteutushyväksyntää): fyysisen Macin laitetesti, huulisynkan ja
taiteen laatu, mallien jakelulisenssit — ne kirjataan laitetestausmenettelyn mukaan.

## Haararekisteri

| Haara | PR | Luokka | CI | Tila |
| --- | --- | --- | --- | --- |
| feat/bundled-speech-models | #163 | ship | vihreä | yhdistetty `1e50666` |
| feat/principles-arcs-frames | #164 | ship | vihreä | yhdistetty `e24be08` |
| feat/cutout-characters | #162 | ship | vihreä (macOS-työ uudelleenajettu 2×, ajokone puuttui) | yhdistetty `1f68ad7` |
| feat/city-library | #165 | ship | vihreä korjauksen `edd6cf4` jälkeen | yhdistetty `1f18c48` |
| fix/city-props-ground-line | #178 | ship | vihreä | yhdistetty `959e828` (2026-11-04) |
| docs/progress-ipad-phase3 | #184 | ship | vihreä | yhdistetty `690966f` |
| feat/five-minute-streaming-export | #181 | ship | vihreä (macOS-työ uudelleenajettu, ajokone puuttui) | yhdistetty `060340e` |
| feat/ipad-timeline-touch | #185 | ship | vihreä | yhdistetty `1cd48ae` |
| feat/comprehensive-library | #182 | ship | vihreä (macOS-työ uudelleenajettu) | yhdistetty `ee03325` |
| feat/albus-yaml-source | #179 | ship | vihreä (macOS-työt uudelleenajettu) | yhdistetty `a33cc18` |
| feat/yaml-pro-tooling | #183 | ship | vihreä korjausten `092b4ff`, `7da2743`, `a4fc255`, `705bcd5` jälkeen | yhdistetty `82d8910` |
| ci/ipad-main-artifact | #186 | ship | vihreä | yhdistetty `a115e4e` |
| feat/night-room-grading | – (vain paikallinen) | defer | – | konflikti mainin kanssa; käyttäjä jätti 1.0:n ulkopuolelle |
| feat/a02-motionscript-compiler | #102 suljettu | defer | – | koskematta |
| feat/direct-manipulation-ui | #93 | stale | – | sisältö mainissa; ei poistettu (käyttäjän päätös) |
| docs/desktop-main-ci | #94 | stale | – | sama |
| docs/mac-gpu-rhubarb-evidence | #47 | stale | – | sama |
| feat/polygon-mask-feather | #48 | stale | – | sama |
| worktree-agent-* (29, paikalliset) | – | stale | – | tyhjiä; ei poistettu |

## Raportti

Jokaisen ajon lopuksi: tila, yhdistetyt PR:t (URL), oikeasti ajetut tarkistukset
ja CI-ajot, `kilsatstudio1.0`:n commit, artefaktit, avoimet asiat ja estot
(esim. puuttuvat tunnukset tai GitHub-oikeudet), seuraavaksi.
