# ORK — Orkestroija

Tila: **todo** · Portit: G0–G4 · Lähtö: commit `496a8a4`

## Tavoite

Ohjata agentit A01–A10 porttien G0–G4 läpi niin, että rajapinnat sovitaan ennen
toteutusta, rinnakkaiset työt eivät kirjoita samoihin tiedostoihin ja jokainen
`done`-merkintä perustuu todennettavaan näyttöön.

## Lähtötiedot

- [Arkkitehtuuriraportti](../KILSAT-STUDIO-ARKKITEHTUURI-JA-AGENTIT.md), luvut 6–10
- [Agenttien README](README.md) ja tehtävätiedostot A01–A10
- [A01-päätökset](../a01-architecture.md), [riippuvuudet](../a01-dependencies.md)
- [master-progress](../master-progress.md) ja [automaattiohjauksen suunnitelma](../automation/README.md)

## Sallittu muutosalue

- `docs/agents/**` (tilat, rekisteri, yhteenvedot)
- `docs/master-progress.md` (vain hyväksytyn yhdistämisen jälkeen, AGENTS.md:n mukaan)
- Ei tuotantokoodia; koodimuutokset tekee omistava agentti.

## Tehtävärekisteri

| ID | Omistaja | Riippuu | Tehtävä | Tila |
| --- | --- | --- | --- | --- |
| KS-001 | A01 + ORK | – | G0: koodikartoitus, riippuvuudet, No AI -politiikka, UI-linjaus | review |
| KS-002 | A02, A03, A04 (+A05/A07/A08) | KS-001 | G1: SceneSpec/TrackSpec/EventLog/AudioTimeline-sopimukset | todo |
| KS-003 | A04–A08 | KS-002 | G2: pienin toimiva kohtaus | todo |
| KS-004 | A09, A10 | KS-003 | G3: käyttöliittymä ja koko ketju käsikirjoitus → video | todo |
| KS-005 | A10 + ORK | KS-004 | G4: CI, macOS-käynnistys, vienti, mittaukset | todo |

## Tehtävät — tarkistuslista

### G0 — Nykytila
- [ ] Lue luvut 6–10, kaikki agenttitiedostot ja nykyinen `master-progress.md`.
- [ ] Vahvista A01:n katselmus (ADR-A01-01…05) tai palauta korjattavaksi.
- [ ] Päätä raportin ja nykykoodin erot: egui vs. SwiftUI (ADR-A01-01), 10 vs. 13 viseemiä,
      RETE-kirjasto vs. nykyinen `expand_rules`, `services/head-pose` (Apple Vision) No AI -luokitus.
- [ ] Sovi päällekkäisyys [automaattiohjauksen agenttien](../automation/README.md) kanssa.

### G1 — Sopimukset
- [ ] Määritä jokaiselle sopimukselle omistava crate: SceneSpec/TrackSpec (A02/A04),
      EventLog (A03), AudioTimeline (A07), rig-kiinnitys (A05), renderöintisopimus (A08).
- [ ] Hyväksy yhteinen versio ja esimerkit; kirjaa ne `docs/agents/`-tiedostoihin.
- [ ] Merkitse G2-tehtävät `ready`-tilaan vasta sopimusten hyväksynnän jälkeen.

### G2 — Pienin toimiva kohtaus
- [ ] Jaa tiedostot ilman päällekkäisiä kirjoittajia (A04 motion-scene, A05 motion-rig,
      A06 motion-animation-fysiikkamoduuli, A07 ääni-cratet, A08 motion-render/motion-art).
- [ ] Käynnistä rinnakkain vain riippumattomat työt; kirjaa jokaisen haara.
- [ ] Tarkista näyte: kaksi tikku-ukkoa, tausta, puhelin kädessä, kameravaihdos, WAV-puhe.

### G3 — Integrointi
- [ ] A09 liittää käyttöliittymän; A10 ajaa koko ketjun ja virhe-/peruutustilanteet.
- [ ] Palauta epäonnistunut työ omistajalle kirjatulla syyllä.

### G4 — Julkaisunäyttö
- [ ] Kokoa A10:n raportti: testit, vertailukuvat, macOS-käynnistys, vienti, mittaukset.
- [ ] Hyväksy vain osoitetut ominaisuudet; kirjaa rajaukset.
- [ ] Päivitä raportin luvun 8 tarkistuslista näytön perusteella.

### Jatkuvat
- [ ] Rajapintamuutos avaa siitä riippuvat tarkistukset uudelleen.
- [ ] Jokaisesta luovutuksesta luvun 10.2 yhteenveto.
- [ ] Ei yhdistämistä `main`-haaraan ilman käyttäjän nimenomaista lupaa.

## Hyväksymisehdot

- Jokaisella `done`-tehtävällä on linkitetty näyttö (commit, testiajo, CI-ajo, kuva tai raportti).
- Yksikään portti ei avaudu ennen edellisen portin hyväksyntää; poikkeukset kirjataan.
- Rekisterissä ei ole tehtävää ilman omistajaa, riippuvuuksia ja hyväksymisehtoa.

## Lopputulema

Odotettu: ajan tasalla oleva tehtävärekisteri, portti kerrallaan hyväksytyt sopimukset ja
toteutukset, käyttäjälle raportoitu tila (toteutettu / tarkistettu / estetty / seuraavaksi).

### Toteutunut lopputulema

_Ei vielä aloitettu._

## Kopioitava tehtävänanto

> Toimi Kilsat Studion orkestroijana. Lue `AGENTS.md`, `docs/agents/README.md`,
> `docs/agents/00-orkestroija.md` ja raportin luvut 6–10. Aloita nykytilan tarkistuksesta
> ja päivitä tehtävärekisteri. Sovi A01:n kanssa moduulirajat ja sopimukset ennen
> toteutusta. Anna jokaiselle agentille sen tehtävätiedosto, sallittu muutosalue,
> riippuvuudet ja hyväksymisehto. Käynnistä rinnakkain vain riippumattomat työt, yksi
> kirjoittaja tiedostoa kohden. Älä merkitse tehtävää valmiiksi pelkän raportin perusteella.
