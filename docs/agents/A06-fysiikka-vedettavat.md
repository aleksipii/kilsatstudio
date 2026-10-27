# A06 — Fysiikka ja vedettävät raajat

Tila: **ready** (G2 KS-003b, haara `feat/a06-springs`; tiedostot [ORK-G2-01](00-orkestroija.md#ork-g2-01--g2n-tiedostoalueet-a05-a06-ja-a08-27102026)) · Portti: G2 · Tehtävä: KS-003 · Lähtö: commit `496a8a4`

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

Tarkka jako: [ORK-G2-01](00-orkestroija.md#ork-g2-01--g2n-tiedostoalueet-a05-a06-ja-a08-27102026).

- Uusi moduuli `crates/motion-animation/src/spring.rs` ja `tests/spring.rs`; `lib.rs`:ään vain
  `pub mod spring;` ja sen uudelleenviennit (muu `lib.rs` jäädytetty G2:n ajaksi; `clips.rs` on A05:n).
- Uusi `docs/physics-springs.md` (sopimus, profiilit, dangle-ehdotus).
- Ei `motion-script`-riippuvuutta: vapautus ottaa tavallisia arvoja; EventLog → alkuarvot on A04:n (KS-003d).
- Ei uutta cratea, ei `animato`-riippuvuutta ilman A01:n päätöstä.

## Riippuvuudet

- Edeltää: A04 (aikajana), A03 (EventLog vapautukselle).
- Sopii yhdessä: A05 (IK-kohde), A09 (raahaus käyttöliittymässä).

## Tehtävät — tarkistuslista

- [ ] `SpringParams { mass, stiffness, damping }` validoinnilla (äärelliset, positiiviset)
      ja johdetut `ω_n`, `ζ`.
- [ ] Suljetun muodon ratkaisu `x(t), v(t)` alkuarvoilla `x0`, `v0` kolmelle alueelle:
      alivaimennettu, kriittinen, ylivaimennettu.
- [ ] Jatkuvuus lähellä ζ = 1: käytä kriittistä kaavaa tai sarjakehitelmää kynnyksen
      `|ζ − 1| < ε` sisällä; testaa, ettei hyppyä tai NaN:ia synny.
- [ ] Huomioi: alkunopeus voi aiheuttaa ylityksen myös kriittisessä vaimennuksessa (testi).
- [ ] Vakiovoima (painovoima): siirretty tasapaino `x_eq = F / k`, analyyttisesti.
- [ ] Aikariippuva voima (tuuli, turbulenssi): määritä deterministinen kiinteän askeleen
      integraattori, jonka askel ei riipu ruutunopeudesta; tila lasketaan ajasta `t`
      (tarvittaessa välitallennuspisteillä), ei edellisestä ruudusta.
- [ ] Profiilit raportin taulukosta vakioiksi; testit tarkistavat ζ-arvot.
- [ ] Vapautus: raahauksen loppuasento ja nopeus (rullaava ikkuna EventLogista) → jousen
      alkuarvot; ei nykäystä vapautushetkellä (testi jatkuvuudesta).
- [ ] Dangle-sopimus: mitkä rig-nivelet voivat saada toissijaisen liikkeen (A05:n kanssa).

## Hyväksymisehdot

- Kaikki tulokset äärellisiä koko parametrialueella; liike jatkuva ζ ≈ 1 ympärillä.
- `x(t)` samalla syötteellä aina sama, riippumatta siitä, missä järjestyksessä aikoja kysytään.
- `cargo test -p motion-animation` ja työtilan tarkistukset läpäisevät.

## Lopputulema

Odotettu: `spring`-moduuli analyyttisillä ratkaisuilla, ulkoisten voimien määritelty käsittely,
profiilit, vapautussopimus ja testit; liitos A05:lle ja A09:lle.

### Toteutunut lopputulema

_Ei vielä aloitettu._

## Kopioitava tehtävänanto

> Toimi agenttina A06. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A06-fysiikka-vedettavat.md`. Toteuta fysiikka puhtaana ajan funktiona
> `motion-animation`-crateen. Tee tarkistuslistan tehtävät, aja työtilan tarkistukset
> ja päivitä tarkistuslista sekä Toteutunut lopputulema.
