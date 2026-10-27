# A04 — Aikajana ja determinismi

Tila: **review** (haara `feat/a04-timeline`; TrackSpec ORK-G1-02 `motion-scene/src/tracks.rs`) · Portit: G1–G2 · Tehtävät: KS-002, KS-003 · Lähtö: commit `496a8a4`

## Tavoite

Ratkaista raitamäärittelyt (TrackSpec) konkreettisiksi raidoiksi, päällekkäisyydet,
suhteelliset/absoluuttiset/palautus-arvot (Rel/Abs/Revert) ja ajan satunnaishaku niin,
että `scene_at_time` on puhdas ajan funktio.

## Lähtötiedot

- `crates/motion-scene/src/lib.rs` — `evaluate(scene_index, time_seconds)`,
  `EvaluatedScene`, `CameraPose`, `frame_count`
- `crates/motion-scene/src/editable_timeline.rs`, `sequence.rs`, `tests/`
- `crates/motion-animation/src/lib.rs` — `PoseTrack`, `blend_pose`, easing
- [editable-timeline.md](../editable-timeline.md), [scene-sequences.md](../scene-sequences.md),
  [workstreams/animation.md](../workstreams/animation.md)

## Nykytila (koodista)

- Arviointi ottaa eksplisiittisen ajan; esikatselu ja vienti käyttävät samaa arvioijaa.
- `PoseTrack`-arvot korvaavat paikalliset muunnokset (ei kumulatiivisia offsetteja).
- Rel/Abs/Revert-käsitteitä ei ole nimetty; raportin kumulatiivinen kaava
  `Pos_base + ΣΔPos_prior + lerp(...)` ei ole nykyinen semantiikka.

## Sallittu muutosalue

- `crates/motion-scene/src/**` ja testit; `docs/editable-timeline.md`
- `motion-animation`-muutokset vain sovitusti A05/A06:n kanssa.
- Ei toista kelloa tai arvioijaa (AGENTS.md).

## Riippuvuudet

- Edeltää: A01, A02 (SceneSpec), A03 (EventLog).
- Kuluttajat: A05, A06, A07 (näytekello), A08, A09, A10.

## Tehtävät — tarkistuslista

- [x] Kirjaa TrackSpec-sopimus: miten nykyiset tapahtumat ja klipit vastaavat
      Abs- ja Rel-raitoja; määritä Revert (paluu perustilaan) eksplisiittisesti.
      → [editable-timeline.md § TrackSpec](../editable-timeline.md#trackspec-overlap-order-and-time-conversion-a04),
      `TrackTarget::{Abs, Rel, Revert}`; Revert = raidan perusarvo.
- [x] Rel-raitojen ratkaisu käännösvaiheessa konkreettisiksi `{from, to}`-väleiksi
      (entiteetti, ominaisuus) -pareittain, jotta toistossa vain interpoloidaan.
      → `ResolvedTrack::resolve` valmisteluvaiheessa; `Event::Move` käyttää sitä arvioijassa.
- [x] Päällekkäiset raidat samalle ominaisuudelle: määritä järjestys (esim. myöhempi alku
      voittaa, tasatilanteessa kirjoitusjärjestys) ja testaa.
      → myöhempi alku voittaa, tasatilanteessa myöhemmin kirjoitettu; nykyisten
      ominaisuuksien säännöt taulukoitu ja testattu.
- [x] Kirjaa ajan muunnokset: sekunnit ↔ 24 kHz:n näytekello ↔ ruutu (`t = frame / fps`),
      pyöristyssääntö ja kohtausoffsetit. → `motion_scene::time`; sama kaava kuin
      `motion-export`issa (testattu).
- [x] Ekvivalenssitesti: suora aikahyppy `t` vs. peräkkäinen toisto ruuduittain samaan `t`:hen
      → sama `EvaluatedScene` (bitti-identtinen tai kirjattu toleranssi). → bitti-identtinen.
- [x] Rajatapaukset: `t` = 0, kohtauksen loppu, kohtausraja, negatiivinen ja ei-äärellinen `t`.
      → myös `-0.0` (normalisoidaan) ja kohtauksen ulkopuolinen näyte.
- [x] Erottele determinismin tasot (ADR-A01-04): tila / pikselit / pakattu video.
      → vain tila testattu A04:ssä; pikselit ja video A08/A10.

## Hyväksymisehdot

- Aikahypyn ja normaalin toiston tulos vastaa toisiaan sovitulla toleranssilla (testi).
- Rel/Abs/Revert ja päällekkäisyyssäännöt ovat dokumentoitu ja testattu.
- Nykyiset motion-scene- ja motion-export-testit läpäisevät.

## Lopputulema

Odotettu: dokumentoitu `scene_at_time`-rajapinta, TrackSpec-sopimus, ekvivalenssi- ja
rajatapaustestit sekä ajan muunnossäännöt A07/A10:lle.

### Toteutunut lopputulema

### A04 / KS-002c — 27.10.2026
- Tila: review
- Omistaja: A04
- Tehty: haara `feat/a04-timeline` (`main` `40b6cd5`:n päällä), vain `crates/motion-scene`
  ja `docs/editable-timeline.md`.
  - Uusi `src/tracks.rs`: `TrackTarget::{Abs, Rel, Revert}`, `AuthoredSpan`, `ResolvedSpan`
    (`from`/`to`, alku- ja loppunäyte), `ResolvedTrack::{resolve, active, value_at}`,
    `resolve_moves`, `MAX_TRACK_SPANS`. Päällekkäisyys: myöhempi alku voittaa ja keskeyttää,
    tasatilanteessa myöhemmin kirjoitettu; valmis väli pitää tarkan `to`-arvon.
  - Arvioija: `Event::Move` ratkaistaan kerran `PreparedAnimation`issa; arviointi vain
    interpoloi (ei enää ketjutusta joka kutsulla). Liikkeiden loppupisteet ovat nyt tarkkoja;
    vanhaan ketjuun verrattuna ero on enintään pyöristys (testi ≤ 1e-9).
    `-0.0` normalisoidaan `0.0`:ksi. Uusi `ValidatedScript::evaluate_sample`.
  - Uusi `src/time.rs`: `seconds_to_sample`, `sample_to_seconds`, `frame_to_sample`,
    `frame_to_seconds`, `sample_to_frame`; `SEQUENCE_SAMPLE_RATE` siirretty sinne
    (julkinen polku `motion_scene::SEQUENCE_SAMPLE_RATE` ennallaan). `sequence.rs` käyttää
    samaa pyöristystä. Uutta kelloa tai arvioijaa ei luotu.
  - Testit: `tests/timeline_determinism.rs` (6), `time.rs` (4), `tracks.rs` (4).
  - Sopimus, kuvaustaulukko, muunnossäännöt ja determinismin tasot:
    [editable-timeline.md](../editable-timeline.md#trackspec-overlap-order-and-time-conversion-a04).
- Tarkistettu (macOS, paikallisesti): `cargo fmt --all --check` ok; `cargo check --workspace` ok;
  `cargo clippy --workspace --all-targets` 0 varoitusta; `cargo test --workspace` ok
  (884 läpäisi, 0 epäonnistui, 8 ohitettu). Nykyiset motion-scene- ja motion-export-testit
  läpäisevät muuttumattomina. CI: ks. PR.
- Tekemättä: kirjoitettu `Rel`/`Revert`-liike puuttuu MotionScriptistä (skeemapyyntö A02:lle);
  muut ominaisuudet (tunteet, taustat, kamera, eleet, klipit) käyttävät omia arvioijiaan,
  joiden säännöt on kirjattu ja testattu, mutta joita ei ole siirretty `ResolvedTrack`iin;
  pikseli- ja videotason determinismi (A08/A10).
- Estot ja riskit: ORK-G1-04:n mukaan kanoninen kello kuuluu `motion-audio`on (A07);
  `motion_scene::time` siirretään delegoimaan siihen, kun A07:n kello on yhdistetty.
  Ruutuaika lasketaan näytekellon kautta (`floor`); jos fps ei jaa 24 000:ta, ruutuaika
  poikkeaa arvosta `frame / fps` alle yhden näytteen verran.
- Seuraavaksi: käyttäjän katselmus ja yhdistämislupa; ORK:n G1-katselmus (KS-002c);
  A07 (kello `motion-audio`on, AudioTimeline v1) voi edetä rinnakkain.

## Kopioitava tehtävänanto

> Toimi agenttina A04. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A04-aikajana-determinismi.md`. Älä luo toista kelloa tai arvioijaa.
> Tee tarkistuslistan tehtävät, aja työtilan tarkistukset ja päivitä tarkistuslista
> sekä Toteutunut lopputulema.
