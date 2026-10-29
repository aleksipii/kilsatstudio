# P03 — Animaatioperiaatteet komentoina

Tila: **todo** · Aalto 2 (P01:n jälkeen) · Haara: `feat/pro-p03-principles` · Lähtö: `main`

## Tavoite

Disneyn periaatteet ovat käsikirjoittajan säädettäviä MotionScript-muuttujia, eivät
koodiin kovakoodattuja vakioita:
- ennakointi
- ylilyönti ja asettuminen
- jatkuva liike ja limittäinen liike
- liikkuva pito
- venytys ja litistys

## Lähtötiedot

- `crates/motion-animation/src/rig_motion.rs`: vakiot `ANTICIPATION_END`, `ANTICIPATION`,
  `OVERSHOOT` ja follow-through-syvyys
- `crates/motion-animation/src/spring.rs` (A06:n analyyttiset jouset)
- `crates/motion-animation/src/clips.rs`
- [character-animation.md](../character-animation.md), [animation-rules.md](../animation-rules.md)
- Tutkimus: [§3](ammattistudiotutkimus.md#3-animointitekniikat)

## Nykytila (koodista, tarkista)

- Ennakointi, ylilyönti ja jatkoliike ovat olemassa kiinteinä vakioina rig-liikkeissä.
- Huomio: päähakemistossa on keskeneräisiä muutoksia `rig_motion.rs`:ään ja `clips.rs`:ään
  haaralla `feat/story-studio`. Aloita vasta, kun ne on yhdistetty tai PRO-orkestroija
  on sopinut järjestyksen.

## Sallittu muutosalue

- `crates/motion-animation/src/rig_motion.rs` ja uusi moduuli `motion-animation/src/principles.rs`
- MotionScript-kytkentä uuteen moduuliin `motion-script/src/principles.rs`; kuuma tiedosto
  `lib.rs`/`story.rs` varataan PRO-orkestroijalta
- Ei muutoksia `spring.rs`:n julkiseen sopimukseen ilman A06:n katselmusta.

## Riippuvuudet

- P01 (easingit `Back`, `EaseInOut`). `feat/story-studio` yhdistetty tai sovittu.

## Tehtävät — tarkistuslista

- [ ] `Principles { anticipation, overshoot, follow_through, squash_stretch }`. Oletusarvot
      ovat nykyiset vakiot, joten vanha tulos pysyy pikselintarkasti samana (testi).
- [ ] Rajat ja validointi: äärelliset arvot sallitulla välillä. Virhe kertoo kentän ja
      sallitun välin.
- [ ] Liikkuva pito: pitkissä pidoissa hahmo hengittää tai heiluu hieman. Liike on
      deterministinen, ilman satunnaisuutta, ja sen voi kytkeä pois (`moving_hold: off`).
- [ ] Venytys ja litistys: tilavuus säilyy (sx·sy ≈ 1). Liike on rajattu, jotta tikku-ukon
      rakenne ei hajoa.
- [ ] MotionScript: `principles:`-lohko hahmolle ja yksittäiselle toiminnolle sekä
      esiasetukset `subtle`, `cartoon` ja `snappy`.
- [ ] Testit: oletus on sama kuin ennen, aikahyppy on sama kuin ruuduittainen toisto, ja
      rajat on testattu.

## Hyväksymisehdot

- Oletusarvoilla nykyiset esimerkit renderöityvät muuttumattomina.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

_(täytetään työn jälkeen)_

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p03-principles. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P03-animaatioperiaatteet.md. Tarkista ensin, onko P01 yhdistetty mainiin.
Luo worktree haaralle feat/pro-p03-principles, tee tehtävät, aja tarkistukset, avaa PR.
```
