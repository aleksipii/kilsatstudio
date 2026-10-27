# A02 — DSL ja kääntäjä

Tila: **review** (PR #105 yhdistetty `d9db5be`, korvaa PR #102:n; odottaa ORK:n G1-katselmusta; SceneSpec-omistaja ORK-G1-01) · Portti: G1 · Tehtävä: KS-002 · Lähtö: commit `496a8a4`

## Tavoite

Määrittää versioitu MotionScript-skeema sekä `seq`-, `par`- ja `stagger`-semantiikka;
validoida kohteet, ajat ja virheet niin, että virheellinen syöte tuottaa paikannetun virheen.

## Lähtötiedot

- `crates/motion-script/src/lib.rs` — `parse`, `Script::validate`, `Script`, `Scene`,
  `Event::{Move, Emotion, Gesture, Speak}`, `CameraKey`
- `crates/motion-script/src/shots.rs`, `camera_presets.rs`, `auto_direction/`
- [motionscript.md](../motionscript.md), [project-format.md](../project-format.md), `examples/`
- Raportin luku 2.1 (YAML-esimerkki, seq/par/stagger)

## Nykytila (koodista)

- MotionScript YAML jäsennetään `serde_yaml`-kirjastolla; validointi on olemassa.
- Kohtausten väliset siirtymät käyttävät 24 kHz:n sekvenssikelloa.
- `seq`/`par`/`stagger`-primitiivejä ei ole; tapahtumilla on absoluuttiset kohtausajat.
- Raportin `act`/`verb`-syntaksi (`fade_in`, `move_to`, `cam_to`, `highlight`) ei vastaa
  nykyistä skeemaa.

## Päätökset (kirjattu ennen toteutusta, 27.10.2026)

**A02-D1 — seq/par/stagger on kääntäjän syntaksisokeria.** Rakenteet lisätään
versioituun kuoreen (`version: 1`, `script`, `schedules`) ja puretaan nykyisiksi
absoluuttisiksi `Event`-ajoiksi ennen `Script::validate`a. Uutta tapahtumalajia,
aikajanatyyppiä, kelloa tai jäsennintä ei luoda: kuori luetaan samalla `serde_yaml`-
jäsentimellä ja sisempi `script` samalla `motion_script::parse`-funktiolla.
Perusteet: (1) arvioija, editori, vienti ja säännöt kuluttavat jo `Script`iä, joten
kaikki näkevät saman tuloksen; (2) vanha versioton `parse` säilyy ennallaan, joten
vanhat esimerkit eivät muutu; (3) A04 omistaa ratkaistut raidat (ORK-G1-02), joten
A02 ei tee ajoituksesta arvioitavaa tilaa.

**A02-D2 — semantiikka.** Ajat ovat äärellisiä f64-sekunteja kohtauksen alusta;
välit ovat puoliksi avoimia `[alku, loppu)`. Solmun alku on `S`, lapsen kesto `Dᵢ`:

| Solmu | Lapsen `i` alku | Solmun kesto |
| --- | --- | --- |
| `seq` | `S + Σ_{j<i} Dⱼ` | `Σ Dᵢ` |
| `par` | `S` | `max Dᵢ` |
| `stagger(delay)` | `S + i·delay` | `max(i·delay + Dᵢ)` |
| `wait(duration)` | – | `duration` |
| `event(span)` | tapahtuma alkaa `S + event.at` | `span` |

`children` ei saa olla tyhjä; `delay ≥ 0`; `duration` ja `span > 0`; `event.at ∈ [0, span)`;
move/gesture mahtuu spaniin; solmun loppu ≤ kohtauksen kesto (kesto ei kasva).
Tasatilanteessa järjestys on vakaa: ensin `script`in omat tapahtumat, sitten puun
kirjoitusjärjestys. Puhe ei mittaa ääntä; kirjoittaja antaa spanin.

**A02-D3 — virheet.** Tyypitetty `Diagnostic` (`DiagnosticCode`: syntax, schema,
version, reference, time, unsupported, validation, limit), YAML-polku
(`$.schedules["s"].children[1].event.actor`) ja syntaksivirheille rivi/sarake.
Käännös on atominen; rajat 1 MiB, 1024 solmua, syvyys 32.

## Sallittu muutosalue

- `crates/motion-script/src/**` (ei `rules.rs`, joka kuuluu A03:lle), sen testit
- `docs/motionscript.md`, `examples/` (uudet esimerkit)
- Ei uutta jäsennincratea tai kilpailevaa jäsennintä (AGENTS.md).

## Riippuvuudet

- Edeltää: A01 (moduulirajat).
- Sopii yhdessä: A03 (sääntöjen syöte), A04 (TrackSpec/aikajana).
- Kuluttajat: A04, A07, A09, automaattiohjauksen agentti 01.

## Tehtävät — tarkistuslista

- [x] Dokumentoi nykyinen skeema ja sen versio; lisää eksplisiittinen versiokenttä,
      jos sellaista ei ole, taaksepäin yhteensopivasti.
      → `docs/motionscript.md` § Versions: versioton = vanha skripti, `version: 1` = kuori;
      JSON Schema `crates/motion-script/schema/`.
- [x] Päätä: lisätäänkö `seq`/`par`/`stagger` kääntäjän syntaksisokerina, joka puretaan
      nykyisiksi absoluuttisiksi tapahtuma-ajoiksi ennen validointia. Kirjaa päätös.
      → A02-D1 (commit `a4917a3`, ennen toteutusta).
- [x] Määritä semantiikka: `seq` = kesto summautuu; `par` = kesto on maksimi;
      `stagger(delay)` = i:nnen lapsen alku `i·delay`. Puoliksi avoimet välit `[alku, loppu)`.
      → A02-D2.
- [x] Hylkää ei-äärelliset, negatiiviset ja nollakestot sekä tuntemattomat kohteet.
- [x] Erottele virhetyypit: syntaksivirhe, virheellinen viite, tukematon komento.
      Tukematon verbi raportoidaan, ei ohiteta. → `compiler::DiagnosticCode` (8 luokkaa).
- [x] Virheisiin rivi/sarake tai YAML-polku (esim. `scenes[0].events[3].target`).
      → polku aina; rivi/sarake YAML-syntaksivirheille.
- [x] Testit: jokainen primitiivi, sisäkkäisyys, rajat, jokainen virhetyyppi, sama syöte →
      sama käännetty tulos. → `crates/motion-script/tests/compiler.rs`, 18 testiä.
- [x] Esimerkit `examples/`-kansioon: kelvollinen ja virheellinen syöte odotettuine virheineen.
      → `examples/motionscript-v1/` (1 kelvollinen, 3 virheellistä `# expect:`-otsakkeella).
- [x] Kirjaa yhteinen SceneSpec-kuvaus (mitä kääntäjä luovuttaa A04:lle).
      → `docs/motionscript.md` § SceneSpec handed to A04.

## Hyväksymisehdot

- Skeema on versioitu ja dokumentoitu; vanhat esimerkit jäsentyvät edelleen.
- Virheellinen syöte tuottaa paikannetun, tyypitetyn virheen ilman paniikkia.
- `cargo test -p motion-script` ja työtilan tarkistukset läpäisevät.

## Lopputulema

Odotettu: versioitu skeema, seq/par/stagger-päätös ja toteutus (jos hyväksytty),
paikannetut virheet, esimerkit ja SceneSpec-sopimus A04:lle.

### Toteutunut lopputulema

### A02 / KS-002a — 27.10.2026
- Tila: review
- Omistaja: A02
- Tehty: haara `feat/a02-dsl` (`main` `331f9d4`:n päällä), korvaa konfliktissa olevan PR #102:n.
  `a4917a3` päätös A02-D1…D3; `f0146f7` PR #102:n kääntäjä (`compiler.rs`,
  `compile`/`compile_with_origins`, `Compilation`) rebasettuna, JSON Schema siirretty
  `crates/motion-script/schema/`-kansioon, määrittely `docs/motionscript.md`:hen,
  `master-progress.md`-muutos jätetty pois (päivitetään vasta yhdistämisen jälkeen);
  `66dfce0` tyypitetty `DiagnosticCode`, kuoren kenttävirheet `schema`-luokkaan
  (aiemmin väärin `syntax`), 10 uutta testiä ja kaksi virheellistä esimerkkiä.
  `rules.rs`iä ei muutettu; uutta jäsennintä ei luotu.
- Tarkistettu (macOS, paikallisesti): `cargo fmt --all --check` ok; `cargo check --workspace` ok;
  `cargo clippy --workspace --all-targets` 0 varoitusta; `cargo test --workspace` ok
  (0 epäonnistunutta); `cargo test -p motion-script` 93 + 9 + 18 ok;
  `compile_script`-esimerkki tuottaa ajat 1, 1, 1.5, 3. Vanhat `examples/*.yaml` (5 kpl)
  jäsentyvät ja tuottavat v1-kuoressa identtisen `Script`in. CI: ks. PR.
- Tekemättä: editorin suora v1-avaus (A09); semanttisten virheiden rivi/sarake (vain polku);
  vanhan `script`-osan sisäiset virheet paikantuvat vain polkuun `$.script`;
  `examples/night-room-reference/*.yaml` eivät ole yksittäisiä skriptejä, eikä niitä testata tässä.
- Estot ja riskit: PR #102 jää auki päällekkäisenä; sulkeminen käyttäjän päätöksellä.
  Tilarivi voi konfliktoida haaran `docs/ork-g1` kanssa (sama rivi).
- Seuraavaksi: käyttäjän katselmus ja yhdistämislupa; sen jälkeen A03 (EventLog v1,
  `pub mod event_log;` `lib.rs`iin) ja A04 (TrackSpec `motion-scene`ssä).

## Kopioitava tehtävänanto

> Toimi agenttina A02. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A02-dsl-kaantaja.md`. Tee tarkistuslistan tehtävät vain sallitulla
> muutosalueella. Älä luo uutta jäsennintä. Aja työtilan tarkistukset ja päivitä
> tarkistuslista sekä Toteutunut lopputulema.
