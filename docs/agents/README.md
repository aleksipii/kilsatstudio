# Kilsat Studio — arkkitehtuuriagentit A01–A10 ja orkestroija

Päivitetty 27.10.2026 (tilat: ORK-G0/G1, ORK-G1-KS-002d, `main` `da5d8d7`). Lähtö: `main`, commit `496a8a41bb753011222764cec9f19c3f3f2b6e89`.
Lähde: [arkkitehtuuriraportti](../KILSAT-STUDIO-ARKKITEHTUURI-JA-AGENTIT.md), luvut 6–10,
sekä [A01-arkkitehtuuripäätökset](../a01-architecture.md). G1-sopimusten omistavat cratet:
[ORK-G1-01…06](00-orkestroija.md#ork-g1--sopimusten-omistajat-27102026).

Tämä kansio jakaa raportin luvun 7 agentit omiksi tehtävätiedostoikseen. Jokaisessa
tiedostossa on tavoite, koodista tarkistettu lähtötila, sallittu muutosalue,
riippuvuudet, **tehtävien tarkistuslista**, hyväksymisehdot, **lopputulema** ja
kopioitava tehtävänanto. Tiedostot ovat tehtävänantoja: niiden olemassaolo ei tarkoita,
että agentti olisi ajettu tai ominaisuus toteutettu.

Erillinen [automaattiohjauksen agenttisarja](../automation/README.md) (01–10) koskee
käsikirjoituksesta ohjattua tuotantoa. Se on eri jako; päällekkäiset rajapinnat sovitaan
orkestroijan kautta eikä kumpikaan sarja luo kilpailevaa jäsennintä, kelloa tai arvioijaa.

## Agentit

| ID | Tiedosto | Pääalue (crate / polku) | Portti | Tila |
| --- | --- | --- | --- | --- |
| ORK | [Orkestroija](00-orkestroija.md) | tehtävärekisteri, `docs/master-progress.md` | G0–G4 | running (G0 hyväksytty, G1 omistajat päätetty) |
| A01 | [Arkkitehtuuri ja riippuvuudet](A01-arkkitehtuuri-riippuvuudet.md) | `docs/a01-*`, manifestit | G0 | done (G0; KS-006/KS-007 siirretty G4:ään) |
| A02 | [DSL ja kääntäjä](A02-dsl-kaantaja.md) | `crates/motion-script` | G1 | review (PR #105 yhdistetty `d9db5be`; ORK:n G1-katselmus) |
| A03 | [Säännöt ja tapahtumat](A03-saannot-tapahtumat.md) | `motion-script/src/rules.rs`, EventLog | G1 | review (PR #106 yhdistetty `6ddfdcc`; ORK:n G1-katselmus) |
| A04 | [Aikajana ja determinismi](A04-aikajana-determinismi.md) | `crates/motion-scene` | G1–G2 | review (PR #107 yhdistetty `10135ad`; ORK:n G1-katselmus) |
| A05 | [Riggaus, IK ja esineet](A05-riggaus-ik-esineet.md) | `crates/motion-rig`, `motion-animation`, käsiesineet | G2 | todo |
| A06 | [Fysiikka ja vedettävät raajat](A06-fysiikka-vedettavat.md) | `crates/motion-animation` (uusi moduuli) | G2 | todo |
| A07 | [Suomen ja englannin ääni](A07-aani-fi-en.md) | `motion-audio`, `motion-lipsync`, `motion-tts`, `motion-voice` | G1–G2 | done G1-osuudelta (PR #109 `ae39c4d`, ORK-G1-KS-002d); G2: KS-002e todo |
| A08 | [Renderöinti, kamera ja taustat](A08-renderointi-kamera.md) | `crates/motion-render`, `motion-art` | G2 | todo |
| A09 | [macOS ja käyttöliittymä](A09-macos-kayttoliittyma.md) | `apps/studio-desktop` | G3 | todo |
| A10 | [Vienti, CI ja laadunvarmistus](A10-vienti-ci-laatu.md) | `crates/motion-export`, `.github/workflows` | G3–G4 | todo |

Tilat: `todo`, `ready`, `running`, `review`, `blocked`, `done`. Vain orkestroija muuttaa
tilan `done`-tilaksi ja vain todennettavan näytön perusteella.

## Portit ja riippuvuudet

```text
G0 Nykytila      A01 ──────────────────────────────┐
G1 Sopimukset    A02 ─ A03 ─ A04   (A05/A07/A08 kommentoivat)
G2 Pienin kohtaus     A04 · A05 · A06 · A07 · A08   (rinnakkain, omat tiedostot)
G3 Integrointi             A09 ─ A10
G4 Julkaisunäyttö                A10 + ORK
```

## Yhteinen toimintaohje kaikille agenteille

1. Lue juuren [AGENTS.md](../../AGENTS.md), tämä README, oma tehtävätiedosto ja
   [A01-päätökset](../a01-architecture.md). Tarkista nykyinen koodi ennen muutoksia;
   älä oleta raportin kirjastojen tai rajapintojen olevan olemassa.
2. Muuta vain omaa sallittua aluetta. Yhteiset tyypit ja rajapinnat sovitaan orkestroijan
   kautta ennen muutosta. Yhdellä tiedostolla on kerrallaan yksi kirjoittaja.
3. Älä lisää raportin ehdokaskirjastoja (`broadsheet`, `rust-rule-engine`, `animato`,
   `rusty_skelform`) ilman A01:n kirjattua riippuvuuspäätöstä.
4. Älä luo tyhjiä crateja suunnitelman nimien takia (ADR-A01-02).
5. Aja Rust-muutoksille työtilan juuresta:
   `cargo fmt --all --check`, `cargo check --workspace`,
   `cargo clippy --workspace --all-targets`, `cargo test --workspace`.
   Jos työkalu puuttuu, kirjaa tarkka estetty tarkistus.
6. Päivitä oman tiedostosi tarkistuslista ja **Toteutunut lopputulema** -osio.
   Rasti tehtävä vain, kun muutos on tehty ja tarkistettu.
7. Palauta orkestroijalle raportin luvun 10.2 yhteenveto:

```markdown
### [Vaihe / tehtävätunnus] — [päivämäärä]
- Tila: todo | ready | running | review | blocked | done
- Omistaja:
- Tehty: konkreettiset muutokset ja tiedostot.
- Tarkistettu: suoritetut tarkistukset, tulokset ja näyttölinkit.
- Tekemättä: avoimet tehtävät ja täyttymättömät hyväksymisehdot.
- Estot ja riskit: yksilöity syy sekä ratkaisemisesta vastaava rooli.
- Seuraavaksi: seuraava tehtävä, omistaja ja riippuvuudet.
```

## Agenttien käynnistäminen Claude Codessa

Jokaiselle roolille on ohut aliagenttimäärittely kansiossa
[`.claude/agents/`](../../.claude/agents/) (esim. `kilsat-a05-rig`). Määrittely ohjaa
agentin lukemaan tämän kansion tehtävätiedoston, joten tehtävät ja tarkistuslistat
ylläpidetään vain täällä.
