# A06 — Fysiikka ja vedettävät raajat

Tila: **review** (KS-003b, PR auki; G2, ORK-G1; EventLogin vapautustapahtumat vasta KS-002f:n jälkeen) · Portti: G2 · Tehtävä: KS-003 · Lähtö: commit `496a8a4`

## Tavoite

Toteuttaa vaimennettujen jousten analyyttiset ratkaisut, alkuarvot (`from_velocity`),
raajojen vapautus ja ulkoisten voimien määritelty käsittely niin, että tulos on ajan
puhdas funktio ja arvot pysyvät äärellisinä myös vaimennussuhteen 1 lähellä.

## Lähtötiedot

- Raportin luvut 1.3, 3.2 ja 3.6 sekä luku 6 (Fysiikka)
- `crates/motion-animation/src/lib.rs` (easing, `Track`), A04:n aikajanasopimus
- A03:n `EventLog` (raahaus- ja vapautustapahtumat), A05:n IK

## Nykytila (koodista)

- Jousi-, vaimennus- tai Dangle-fysiikkaa ei löytynyt mistään cratesta.
- Raportin profiilit (Snappy, Gentle, Bouncy, Stiff, Wobbly) tarkistettu laskemalla:
  ζ = c / (2√(k·m)) antaa 1,00 / 0,61 / 0,28 / 1,00 / 0,19.

## Sallittu muutosalue

- Uusi moduuli `crates/motion-animation/src/spring.rs` (+ testit); vientien lisäys `lib.rs`:ään.
- Ei uutta cratea, ei `animato`-riippuvuutta ilman A01:n päätöstä.

## Riippuvuudet

- Edeltää: A04 (aikajana), A03 (EventLog vapautukselle).
- Sopii yhdessä: A05 (IK-kohde), A09 (raahaus käyttöliittymässä).

## Tehtävät — tarkistuslista

- [x] `SpringParams { mass, stiffness, damping }` validoinnilla (äärelliset, positiiviset)
      ja johdetut `ω_n`, `ζ`.
- [x] Suljetun muodon ratkaisu `x(t), v(t)` alkuarvoilla `x0`, `v0` kolmelle alueelle:
      alivaimennettu, kriittinen, ylivaimennettu.
- [x] Jatkuvuus lähellä ζ = 1: käytä kriittistä kaavaa tai sarjakehitelmää kynnyksen
      `|ζ − 1| < ε` sisällä; testaa, ettei hyppyä tai NaN:ia synny.
- [x] Huomioi: alkunopeus voi aiheuttaa ylityksen myös kriittisessä vaimennuksessa (testi).
- [x] Vakiovoima (painovoima): siirretty tasapaino `x_eq = F / k`, analyyttisesti.
- [x] Aikariippuva voima (tuuli, turbulenssi): määritä deterministinen kiinteän askeleen
      integraattori, jonka askel ei riipu ruutunopeudesta; tila lasketaan ajasta `t`
      (tarvittaessa välitallennuspisteillä), ei edellisestä ruudusta.
- [x] Profiilit raportin taulukosta vakioiksi; testit tarkistavat ζ-arvot.
- [x] Vapautus: raahauksen loppuasento ja nopeus (rullaava ikkuna EventLogista) → jousen
      alkuarvot; ei nykäystä vapautushetkellä (testi jatkuvuudesta). Toteutettu tavallisilla
      arvoilla (`ReleaseSample`); EventLog-kytkentä `motion-scene`n kautta KS-002f:n jälkeen.
- [ ] Dangle-sopimus: mitkä rig-nivelet voivat saada toissijaisen liikkeen (A05:n kanssa).
      Ehdotus kirjattu `docs/physics-springs.md`:hen; A05:n hyväksyntä puuttuu.

## Hyväksymisehdot

- Kaikki tulokset äärellisiä koko parametrialueella; liike jatkuva ζ ≈ 1 ympärillä.
- `x(t)` samalla syötteellä aina sama, riippumatta siitä, missä järjestyksessä aikoja kysytään.
- `cargo test -p motion-animation` ja työtilan tarkistukset läpäisevät.

## Lopputulema

Odotettu: `spring`-moduuli analyyttisillä ratkaisuilla, ulkoisten voimien määritelty käsittely,
profiilit, vapautussopimus ja testit; liitos A05:lle ja A09:lle.

### Toteutunut lopputulema

KS-003b (haara `feat/a06-springs`, 2026-10-27):

- `crates/motion-animation/src/spring.rs`: `SpringParams` (validoitu, ω_n, ζ, `regime`),
  `Spring` (suljettu muoto kaikille kolmelle alueelle yhtenä lausekkeena; Taylorin sarja
  kun `|s|·t² < 1e-4`, joten ζ = 1 on jatkuva ilman kynnyshyppyä), `with_constant_force`
  (tasapaino `target + F/k`), `ForcedSpring` + `ForceProfile`/`Gust`/`Turbulence`
  (kiinteä askel 1/1200 s, tarkka propagaattori keskipisteen voimalla, välitallennus 1 s
  välein, bitilleen sama kuin askellus nollasta), `SpringProfile` (5 profiilia),
  `release_state`/`Spring::from_release` (viimeinen asento + PNS-nopeus ikkunasta),
  `SpringError`. `lib.rs`: `pub mod spring;` ja uudelleenviennit.
- `crates/motion-animation/tests/spring.rs`: 19 testiä (profiilien ζ, validointi,
  ODE-jäännös, jatkuvuus ζ ≈ 1, kriittinen ylitys, painovoima, kyselyjärjestys, aikahyppy,
  äärellisyys ääripäissä, vapautuksen jatkuvuus, pakotetun jousen determinismi ja
  stabiilius jäykimmällä jousella).
- `docs/physics-springs.md`: malli, rajat, vapautussopimus ja Dangle-ehdotus.
- Tarkistettu: `cargo fmt --all --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets` (ei varoituksia), `cargo test --workspace`
  (48 testiajoa, 925 läpäisi, 0 epäonnistui).
- Avoinna: Dangle-sopimus A05:n kanssa, EventLog-kytkentä (KS-002f), A09:n säätimet;
  visuaalista laatua ei ole arvioitu renderöidyssä kuvassa.

## Kopioitava tehtävänanto

> Toimi agenttina A06. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A06-fysiikka-vedettavat.md`. Toteuta fysiikka puhtaana ajan funktiona
> `motion-animation`-crateen. Tee tarkistuslistan tehtävät, aja työtilan tarkistukset
> ja päivitä tarkistuslista sekä Toteutunut lopputulema.
