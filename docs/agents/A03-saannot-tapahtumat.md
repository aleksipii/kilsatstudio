# A03 — Säännöt ja tapahtumat

Tila: **done** G1-osalta (PR #106 `6ddfdcc`; ORK-G1) · palautus KS-002f **ready**: EventLogin `sample_rate` kanoniseen kelloon ([ORK-G1, puute 1](00-orkestroija.md#ork-g1--porttipäätös-hyväksytty-ehdoin-27102026)) · Portti: G1 · Tehtävä: KS-002 · Lähtö: commit `496a8a4`

## Tavoite

Kirjata ja tarvittaessa täydentää sääntöjen vakaa järjestys, prioriteetit ja
konfliktien ratkaisu; estää loputon ketjutus; tallentaa ulkoiset tapahtumat
toistettavaksi tapahtumajäljeksi (EventLog).

## Lähtötiedot

- `crates/motion-script/src/rules.rs`, `rules_tests.rs` — `RuleTrigger`, `RuleCondition`,
  `RuleEffect`, `Script::expand_rules`, `MAX_RULES`, `RuleError`
- [animation-rules.md](../animation-rules.md), [audio-cues.md](../audio-cues.md)
- `apps/studio-desktop/src/cue_rule_editor.rs`, `webcam_pose.rs`, `direct_manipulation.rs`
- Raportin luvut 2.2 ja 3.4 sekä luku 6 (Ajallinen tila, Sääntömoottori)

## Nykytila (koodista)

- `expand_rules` on deterministinen: säännöt nimen mukaan nousevasti, laukaisut ajan ja
  kirjoitusjärjestyksen mukaan; tuotetut klipit `r_<sääntö>_<n>`.
- Vaikutukset ovat klippejä tai kamera-avaimia; ne eivät tuota uusia faktoja, joten
  nykyinen malli ei ketjuunnu.
- RETE-verkkoa, GRL-syntaksia tai yleistä EventLog-rakennetta ei ole.

## Sallittu muutosalue

- `crates/motion-script/src/rules.rs`, `rules_tests.rs`, [animation-rules.md](../animation-rules.md)
- EventLog-tyypin sijainti sovitaan orkestroijan kanssa (ehdokas: `motion-script` tai
  `motion-project`); ei uutta cratea ilman konkreettista vastuuta.
- Ei RETE-kirjaston lisäystä ilman A01:n päätöstä.

## Riippuvuudet

- Edeltää: A01, A02 (skeema).
- Kuluttajat: A04 (aikajana), A06 (vedettävien raajojen vapautus), A07 (viseemisäännöt),
  A09 (näppäin- ja kameratapahtumat).

## Tehtävät — tarkistuslista

- [x] Kirjaa nykyinen sääntösopimus: järjestys, prioriteetti (sääntönimi), päällekkäisyys,
      rajat ja virheluokat. → [animation-rules.md § Rule contract](../animation-rules.md#rule-contract-a03).
- [x] Päätä tarvitaanko eksplisiittinen prioriteettikenttä; jos kyllä, lisää se muuttamatta
      nykyisten skriptien tulosta. → A03-D1: ei kenttää; sääntönimi on prioriteetti.
- [x] Jos ketjuuntuvia sääntöjä (vaikutus → uusi fakta) lisätään, aseta kiinteä
      ketjusyvyys ja laukaisumäärä; ylitys on tyypitetty virhe.
      → A03-D2: ketjutusta ei lisätty; testi `logged_events_and_rule_outputs_do_not_chain`.
- [x] Määritä `EventLog` v1: tapahtumatyyppi (näppäin, raahaus, kameran asento, vapautus),
      aika näytekellossa, lähde, arvo, versio. Ei piilotettua toistohistoriaa.
      → `motion-script/src/event_log.rs`, [§ EventLog v1](../animation-rules.md#eventlog-v1).
- [x] Toteuta EventLogin serialisointi ja validointi (äärelliset ajat, järjestys, rajat).
      → JSON (`from_json`/`to_json`), `validate`, `validate_for(&Script)`, `EventLogError`.
- [x] Testi: sama skripti + sama EventLog → sama laukaisujärjestys ja sama laajennettu skripti.
      → `same_script_and_log_give_same_firings_and_expansion` (myös JSON-toiston jälkeen).
- [x] Mittaa sääntölaajennuksen kustannus (säännöt × tapahtumat) ja kirjaa tulos laitetietoineen.
      Älä lupaa O(1)-suorituskykyä. → Apple M1, [§ Expansion cost](../animation-rules.md#expansion-cost-measured).

## Hyväksymisehdot

- Sama syöte tuottaa saman laukaisujärjestyksen (testi).
- Ketjutus ei voi jatkua rajatta (testi, jos ketjutus lisätään).
- Ulkoiset tapahtumat ovat syötettä, jonka toistaminen tuottaa saman kohtauksen.
- Nykyiset `rules_tests` läpäisevät muuttumattomina.

## Lopputulema

Odotettu: kirjattu sääntösopimus, `EventLog` v1 testeineen, mittaustulos ja
A06/A09:lle luovutettu tapahtumasopimus.

### Toteutunut lopputulema

### A03 / KS-002b — 27.10.2026
- Tila: review
- Omistaja: A03
- Tehty: haara `feat/a03-rules-eventlog` (`main` `b813727`:n päällä).
  - EventLog v1 ORK-G1-03:n mukaan uuteen moduuliin `crates/motion-script/src/event_log.rs`:
    `EventLog`/`LoggedEvent`/`EventSource`/`ExternalEvent` (key, drag, camera_pose, release),
    kokonaislukuinen kohtauspaikallinen näytepaikka, lokiin kirjattu `sample_rate`, versio 1,
    JSON-sarjallistus, rakenteellinen ja skriptiä vasten tehtävä validointi, `EventLogError`
    polkuineen (`events[i].field`). `lib.rs`:ään vain `pub mod event_log;` ja uudelleenviennit.
  - `rules.rs`: uusi laukaisin `key` (laukeaa vain lokin näppäinpainalluksista),
    `Script::expand_rules_with_events` → `RuleExpansion { script, firings }`, `RuleFiring`,
    `FiringSource`, `RuleError::EventLog`. `expand_rules()` käyttää samaa polkua ilman lokia;
    järjestys, tunnisteet ja virheviestit ennallaan.
  - Tunne-ehdon haku: kohtauskohtainen tunnehistoria + binäärihaku nykyisen koko historian
    läpikäynnin sijaan (sama tulos, mitattu nopeutus).
  - Päätökset A03-D1 (ei prioriteettikenttää) ja A03-D2 (ei ketjutusta); sääntösopimus,
    EventLog v1 ja mittaus `docs/animation-rules.md`:ssä; esimerkit `examples/event-log-v1/`.
  - Mittaus: `crates/motion-script/examples/rule_expansion_cost.rs`.
  - Rajanylitys (A09:n tiedosto): `apps/studio-desktop/src/cue_rule_editor.rs`:ään yksi
    `RuleTrigger::Key`-nimikerivi, jota tyhjentävä `match` vaatii. Editorin laukaisinvalikkoon
    `key`-vaihtoehtoa ei lisätty.
  - `motion-script`: `serde_json` siirretty dev-riippuvuudesta normaaliksi (työtilariippuvuus,
    `Cargo.lock` ei muuttunut).
- Tarkistettu (macOS 27.0.1, Apple M1, paikallisesti): `cargo fmt --all --check` ok;
  `cargo check --workspace` ok; `cargo clippy --workspace --all-targets` 0 varoitusta;
  `cargo test --workspace` 870 läpi, 0 epäonnistunutta, 8 ohitettua.
  `motion-script`: 93 yksikkötestiä (sis. `rules_tests` muuttumattomana, `git diff` tyhjä),
  12 uutta `tests/event_log.rs`-testiä. Mittaus: pahin tapaus 64 sääntöä × 1024 tapahtumaa
  213 ms → 1,5 ms; muut tapaukset ks. dokumentti. CI: ks. PR.
- Tekemättä: lokin tallennus projektitiedostoon ja käyttöliittymästä (A09, G3); drag/release-
  ja camera_pose-tapahtumien kulutus aikajanassa ja fysiikassa (A04/A06); `key`-valinta
  sääntöeditorin valikkoon (A09); lokin näytetaajuuden vertailu kanoniseen kelloon, kun A07:n
  kello on olemassa (ORK-G1-04). Mittaus on yhden koneen tulos ilman kuormanhallintaa.
- Estot ja riskit: tilarivi jätettiin muuttamatta, koska avoin PR #104 (`docs/ork-g1`) muuttaa
  samaa riviä. Lisätty `RuleTrigger::Key`-variantti rikkoo muiden cratejen tyhjentävät
  `match`-lausekkeet (työtilassa vain yksi, korjattu).
- Seuraavaksi: käyttäjän katselmus ja yhdistämislupa; A04 voi aloittaa TrackSpecin
  (EventLog v1 käytettävissä), A07 näytekellon ja AudioTimeline v1:n.

## Kopioitava tehtävänanto

> Toimi agenttina A03. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A03-saannot-tapahtumat.md`. Säilytä `expand_rules`-järjestyksen
> nykyinen semantiikka. Tee tarkistuslistan tehtävät, aja työtilan tarkistukset ja
> päivitä tarkistuslista sekä Toteutunut lopputulema.
