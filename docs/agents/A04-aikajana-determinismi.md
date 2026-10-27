# A04 — Aikajana ja determinismi

Tila: **todo** (odottaa A02 + A03; TrackSpec ORK-G1-02) · Portit: G1–G2 · Tehtävät: KS-002, KS-003 · Lähtö: commit `496a8a4`

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

- [ ] Kirjaa TrackSpec-sopimus: miten nykyiset tapahtumat ja klipit vastaavat
      Abs- ja Rel-raitoja; määritä Revert (paluu perustilaan) eksplisiittisesti.
- [ ] Rel-raitojen ratkaisu käännösvaiheessa konkreettisiksi `{from, to}`-väleiksi
      (entiteetti, ominaisuus) -pareittain, jotta toistossa vain interpoloidaan.
- [ ] Päällekkäiset raidat samalle ominaisuudelle: määritä järjestys (esim. myöhempi alku
      voittaa, tasatilanteessa kirjoitusjärjestys) ja testaa.
- [ ] Kirjaa ajan muunnokset: sekunnit ↔ 24 kHz:n näytekello ↔ ruutu (`t = frame / fps`),
      pyöristyssääntö ja kohtausoffsetit.
- [ ] Ekvivalenssitesti: suora aikahyppy `t` vs. peräkkäinen toisto ruuduittain samaan `t`:hen
      → sama `EvaluatedScene` (bitti-identtinen tai kirjattu toleranssi).
- [ ] Rajatapaukset: `t` = 0, kohtauksen loppu, kohtausraja, negatiivinen ja ei-äärellinen `t`.
- [ ] Erottele determinismin tasot (ADR-A01-04): tila / pikselit / pakattu video.

## Hyväksymisehdot

- Aikahypyn ja normaalin toiston tulos vastaa toisiaan sovitulla toleranssilla (testi).
- Rel/Abs/Revert ja päällekkäisyyssäännöt ovat dokumentoitu ja testattu.
- Nykyiset motion-scene- ja motion-export-testit läpäisevät.

## Lopputulema

Odotettu: dokumentoitu `scene_at_time`-rajapinta, TrackSpec-sopimus, ekvivalenssi- ja
rajatapaustestit sekä ajan muunnossäännöt A07/A10:lle.

### Toteutunut lopputulema

_Ei vielä aloitettu._

## Kopioitava tehtävänanto

> Toimi agenttina A04. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A04-aikajana-determinismi.md`. Älä luo toista kelloa tai arvioijaa.
> Tee tarkistuslistan tehtävät, aja työtilan tarkistukset ja päivitä tarkistuslista
> sekä Toteutunut lopputulema.
