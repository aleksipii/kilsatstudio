# A01 — Arkkitehtuuri ja riippuvuudet

Tila: **done** G0-laajuudessa (ORK-G0, 27.10.2026; PR #101 ja #103). Ei julkaisu- tai lisenssihyväksyntä: avoimet kohdat siirretty KS-006/KS-007:ään (G4).
Portti: G0 · Tehtävä: KS-001 · Lähtö: commit `496a8a4`

## Tavoite

Tarkistaa lähdekoodi, riippuvuudet ja lisenssit; erottaa nykytila tavoitteesta;
määrittää moduulirajat ja No AI -käyttöpolut, joihin muut agentit nojaavat.

## Lähtötiedot

- [a01-architecture.md](../a01-architecture.md) — ADR-A01-01…05
- [a01-dependencies.md](../a01-dependencies.md), [a01-dependency-inventory.json](../a01-dependency-inventory.json)
- `Cargo.toml`, `Cargo.lock`, `crates/*/Cargo.toml`, `apps/studio-desktop/Cargo.toml`
- `services/kokoro-runtime/requirements.txt`, `services/head-pose/`
- `scripts/license_inventory.py`, `tests/test_license_inventory.py`

## Nykytila (koodista)

- 14 workspace-moduulia (13 cratea + `studio-desktop`), normaali riippuvuusgraafi syklitön.
- 15 suoraa ulkoista Rust-riippuvuutta, 434 lukittua pakettia.
- Raportin ehdokaskirjastot (`broadsheet`, `rust-rule-engine`, `animato`, `rusty_skelform`)
  puuttuvat lockista.
- `services/head-pose` käyttää Apple Visionin kasvontunnistusta (koneoppimiseen perustuva
  järjestelmäkirjasto). Sen luokittelu No AI -polkuihin puuttuu.
- `motion-export` riippuu `motion-voice`- ja `motion-tts`-moduuleista.

## Sallittu muutosalue

- `docs/a01-*`, `docs/agents/A01-*.md`, `scripts/license_inventory.py` ja sen testit.
- Ei tuotantokoodia, ei uusia riippuvuuksia.

## Riippuvuudet

- Ei edeltäviä. Kaikki A02–A10 riippuvat A01:n moduulirajoista ja No AI -politiikasta.

## Tehtävät — tarkistuslista

- [x] Lue AGENTS.md, manifestit, lock ja nykyinen riippuvuusauditointi.
- [x] Kirjaa suorien riippuvuuksien perustelut ja tarkistustila.
- [x] Tuota koneellinen inventaario ja aja sen testit (16/16, PR #101).
- [x] Kirjaa ADR-A01-01…05 (egui-lähtökohta, omistus, No AI -polut, determinismin tasot,
      lukitus ≠ lisenssihyväksyntä).
- [x] Luokittele `services/head-pose` (Apple Vision) No AI -polkuihin: tiukassa polussa
      estettävä vai sallittu syötelaite; kirjaa ADR. → ADR-A01-06: tiukassa estetään,
      valinnaisessa ML-polussa sallittu käyttäjän käynnistämänä syötelaitteena.
- [x] Arvioi neljä ehdokaskirjastoa (API, lisenssi, ylläpito, tarve) ja kirjaa
      päätös: hylätty / korvataan omalla toteutuksella / lisätään. Oletus: ei lisätä.
      → ADR-A01-07: `broadsheet` ja `rusty_skelform` hylätty; `rust-rule-engine` ja
      `animato` korvataan omalla toteutuksella (A03, A06). Mitään ei lisätä.
- [x] Kirjaa raportin 10 viseemin ja nykyisen 13 viseemin (`motion-lipsync::Viseme`)
      ero A07:lle päätettäväksi. → ADR-A01-08 (vastaavuustaulukko ja suositus).
- [x] Laadi tiukan No AI -politiikan rajapintaehdotus (tyyppi, omistava crate, tarkistuskohta)
      ja luovuta A07/A09:lle. → ADR-A01-09: `ModelPolicy` + `ModelBackedStep`
      `motion-project`-crateen; ei toteutettu.
- [x] Täydennä kohdekohtainen (macOS aarch64) riippuvuusgraafi `cargo tree --target`-ajolla.
      → 188 ulkoista pakettia (+22 `gpu`, +16 build); [a01-macos-aarch64-packages.txt](../a01-macos-aarch64-packages.txt).
- [ ] Python-wheelien lukitus hasheineen `kokoro-runtime`-palvelulle tai kirjattu este.
      **Este kirjattu** ([a01-open-decisions.md](../a01-open-decisions.md#kirjatut-estot-ei-ratkaistu-tässä-työssä));
      lukitus tekemättä.
- [ ] Kolmannen osapuolen lisenssitekstit ja NOTICE-velvoitteet jaeltaville artefakteille.
      **Este kirjattu**; tekstit keräämättä.

## Hyväksymisehdot

- Jokaisella suoralla riippuvuudella on perustelu ja tarkistustila.
- Moduulirajat ja No AI -polut ovat kirjattu ADR:inä, joihin muut agentit voivat viitata.
- Ehdokaskirjastoista on kirjattu päätös ennen kuin kukaan lisää niitä.
- Avoimet lisenssi- ja jakelukohdat on nimetty, ei merkitty hyväksytyiksi.

## Lopputulema

Odotettu: arkkitehtuuripäätökset, riippuvuuslista ja inventaario; No AI -politiikan
rajapintaehdotus; ehdokaskirjastojen päätökset; avointen lisenssikohtien lista.

### Toteutunut lopputulema

- PR #101 (yhdistetty 26.10.2026, `cdff71d`): ADR:t, riippuvuuslista, 434 paketin
  inventaario, inventaariotestit 16/16. CI (macOS + Ubuntu) läpäisi.
- Haara `docs/a01-open-decisions` (27.10.2026, lähtö `561036c`):
  [a01-open-decisions.md](../a01-open-decisions.md) — ADR-A01-06 (head-pose/Apple Vision
  mallipohjaisena syötteenä), ADR-A01-07 (neljä ehdokasta: ei lisätä), ADR-A01-08
  (10 vs. 13 viseemiä, päätös A07:lle), ADR-A01-09 (`ModelPolicy`-rajapintaehdotus
  A07/A09:lle) sekä macOS aarch64 -graafi `cargo tree --target aarch64-apple-darwin`
  -ajoilla ([pakettilista](../a01-macos-aarch64-packages.txt)). Tuotantokoodia ei muutettu.
- Avoinna (kirjattuina estoina): Python-wheelien hash-lukitus ja kolmannen osapuolen
  lisenssitekstit/NOTICEt. Lisäksi ADR-A01-09:n toteutus on A07/A09:n työtä, ja
  head-pose-avainruutujen alkuperämerkintä puuttuu (A09).

## Tarkistukset

```sh
python3 scripts/license_inventory.py --output docs/a01-dependency-inventory.json
python3 -m unittest discover -s tests -p test_license_inventory.py -v
cargo tree --workspace --target aarch64-apple-darwin -e normal
```

## Kopioitava tehtävänanto

> Toimi agenttina A01. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A01-arkkitehtuuri-riippuvuudet.md`. Täydennä avoimet kohdat
> tarkistuslistasta. Älä muuta tuotantokoodia tai lisää riippuvuuksia. Päivitä
> tarkistuslista ja Toteutunut lopputulema; palauta luvun 10.2 yhteenveto.
