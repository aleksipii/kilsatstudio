# A01 — Arkkitehtuuri ja riippuvuudet

Tila: **review** (katselmus yhdistetty PR #101:ssä; ei julkaisu- tai lisenssihyväksyntä)
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
- [ ] Luokittele `services/head-pose` (Apple Vision) No AI -polkuihin: tiukassa polussa
      estettävä vai sallittu syötelaite; kirjaa ADR.
- [ ] Arvioi neljä ehdokaskirjastoa (API, lisenssi, ylläpito, tarve) ja kirjaa
      päätös: hylätty / korvataan omalla toteutuksella / lisätään. Oletus: ei lisätä.
- [ ] Kirjaa raportin 10 viseemin ja nykyisen 13 viseemin (`motion-lipsync::Viseme`)
      ero A07:lle päätettäväksi.
- [ ] Laadi tiukan No AI -politiikan rajapintaehdotus (tyyppi, omistava crate, tarkistuskohta)
      ja luovuta A07/A09:lle.
- [ ] Täydennä kohdekohtainen (macOS aarch64) riippuvuusgraafi `cargo tree --target`-ajolla.
- [ ] Python-wheelien lukitus hasheineen `kokoro-runtime`-palvelulle tai kirjattu este.
- [ ] Kolmannen osapuolen lisenssitekstit ja NOTICE-velvoitteet jaeltaville artefakteille.

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
- Avoinna: yllä rastittamattomat kohdat.

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
