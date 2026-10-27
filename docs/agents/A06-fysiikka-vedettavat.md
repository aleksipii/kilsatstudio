# A06 — Fysiikka ja vedettävät raajat

Tila: **review** (G2 KS-003b, haara `feat/a06-springs`, PR #115; tiedostot [ORK-G2-01](00-orkestroija.md#ork-g2-01--g2n-tiedostoalueet-a05-a06-ja-a08-27102026); EventLogin vapautustapahtumat vasta KS-002f:n jälkeen) · Portti: G2 · Tehtävä: KS-003 · Lähtö: commit `496a8a4`

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
- [x] Dangle-sopimus: mitkä rig-nivelet voivat saada toissijaisen liikkeen (A05:n kanssa).
      → `dangle_joint`/`apply_dangle` `spring.rs`:ssä, sopimus `docs/physics-springs.md`:ssä
      A05:n kiinnityssopimuksen (PR #117) pohjalta; A05:n katselmus kirjattu alle.

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
- Avoinna: EventLog-kytkentä (KS-002f), A09:n säätimet; visuaalista laatua ei ole
  arvioitu renderöidyssä kuvassa.

Dangle-sopimus (haara `feat/a06-dangle-contract`, 2026-10-27):

- `spring.rs`: `dangle_joint` (sallii vain nivelet, jotka eivät ole juuria, A05:n
  kiinnitysrungon niveliä — `root`, `torso`, `STICK_LIMBS`in 12 niveltä — eivätkä niiden
  esivanhempia), `apply_dangle` (lisää kulman vain kiertoon, `|angle| ≤ π`), `DangleError`.
  `lib.rs`: uudelleenviennit. Nivelrajat puristaa `PreparedRig`in arviointi.
- `tests/spring.rs`: 6 uutta testiä (25 yhteensä), mm. pään jousi vapautuksesta 24 fps:llä
  (raajat pysyvät bitilleen paikallaan, raja puristaa kierron) sekä A05:n ehtona vaatima
  yhteistesti: puhelin oikeassa kädessä, oikean käden IK neljään kohteeseen (yksi
  saavuttamaton) ja jouset päässä ja käden alaisessa avaimenperässä → IK-ratkaisu,
  raajanivelet, `attachment_gap` = 0, otepiste ja puhelimen osat bitilleen samat kuin ilman
  jousia.
- A05:n katselmus (27.10.2026): **hyväksytty ehdoin**; ehto (IK + ote + esine -yhteistesti)
  täytetty yllä. A05:n riskit: nimeen perustuva tunnistus on tarkoituksella konservatiivinen;
  rajalla jousi voi näkyä pysähtymisenä.
- `docs/physics-springs.md`: sopimus, sallitut nivelet, jousen lähteet ja avoimet kohdat.
- Avoinna: dangle-nivelten merkintä MotionScriptiin/rigiin (A02/A05), jälkiliike vanhemman
  animaatiosta (A04, KS-003d), säätimet (A09).

## Kopioitava tehtävänanto

> Toimi agenttina A06. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A06-fysiikka-vedettavat.md`. Toteuta fysiikka puhtaana ajan funktiona
> `motion-animation`-crateen. Tee tarkistuslistan tehtävät, aja työtilan tarkistukset
> ja päivitä tarkistuslista sekä Toteutunut lopputulema.
