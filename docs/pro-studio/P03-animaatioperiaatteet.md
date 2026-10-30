# P03 — Animaatioperiaatteet komentoina

Tila: **review** (PR #151) · Aalto 2 (P01:n jälkeen) · Haara: `feat/pro-p03-principles` · Lähtö: `main`

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

- [x] `Principles { anticipation, overshoot, follow_through, squash_stretch }`. Oletusarvot
      ovat nykyiset vakiot, joten vanha tulos pysyy pikselintarkasti samana (testi).
- [x] Rajat ja validointi: äärelliset arvot sallitulla välillä. Virhe kertoo kentän ja
      sallitun välin.
- [x] Liikkuva pito: pitkissä pidoissa hahmo hengittää tai heiluu hieman. Liike on
      deterministinen, ilman satunnaisuutta, ja sen voi kytkeä pois (`moving_hold: off`).
- [x] Venytys ja litistys: tilavuus säilyy (sx·sy ≈ 1). Liike on rajattu, jotta tikku-ukon
      rakenne ei hajoa.
- [x] MotionScript: `principles:`-lohko hahmolle ja yksittäiselle toiminnolle sekä
      esiasetukset `subtle`, `cartoon` ja `snappy`.
- [x] Testit: oletus on sama kuin ennen, aikahyppy on sama kuin ruuduittainen toisto, ja
      rajat on testattu.

## Hyväksymisehdot

- Oletusarvoilla nykyiset esimerkit renderöityvät muuttumattomina.
- fmt, check, clippy ja test vihreinä.

## Toteutunut lopputulema

Haara `feat/pro-p03-principles`, PR https://github.com/aleksipii/kilsatstudio/pull/151 (ei yhdistetty).

- `motion-animation/src/principles.rs`: `Principles { anticipation, overshoot, follow_through,
  overlap, squash_stretch, moving_hold }`, rajat, validointi (virhe nimeää kentän ja välin) ja
  esiasetukset `subtle`, `cartoon` ja `snappy`. `Principles::DEFAULT` = vanhat vakiot.
- `rig_motion.rs`: `acting_curve_with`, `rig_gesture_with` ja `rig_moving_hold`. Venytys ja litistys
  skaalaavat vain `torso`-niveltä (sx·sy = 1, enintään ±0,3), joten raajat pysyvät kiinni. Liikkuva pito
  käynnistyy 0,5 s pysähdyksen jälkeen: hengitys ja pään heilahdus, ei satunnaisuutta, oletuksena pois.
- `motion-script/src/principles.rs`: `principles:` hahmolle ja gesture-tapahtumalle. Ele kerrostuu
  hahmon päälle. Ilman riggiä tai eleen `moving_hold`-kentän kanssa annetaan unsupported-virhe.
  Tarinamuoto: `cast.*.principles` ja beatin `principles`.
- `motion-scene`: kytkentä sekä skaalan yhdistäminen `add_rig_motion`-funktiossa.
- Dokumentaatio: `docs/animation-principles.md`, `motionscript.md` ja `story-format.md`.
- Tarkistukset ajettu: fmt, check, clippy ja test vihreinä (1140 läpi, 11 ohitettu). Kaikki 76
  esimerkkiskriptiä arvioitiin jokaisella ruudulla mainilla ja haaralla, ja tulokset olivat identtiset.
  Tarinoiden YAML-esimerkit eivät olleet mukana.
- Avoinna: P01:n easingeja ei ole vielä mainissa, joten käyrä käyttää omaa back-kaavaansa (siirto
  P01:n easingeihin on jatkotyö). Vaiheiden ajoitusosuudet ovat kiinteitä. Desktop-säätimiä ei ole.
  Esiasetuksia ei ole tarkastettu silmämääräisesti renderöinnissä.

## Kopioitava tehtävänanto

```text
Toimi agenttina kilsat-p03-principles. Lue AGENTS.md, docs/pro-studio/README.md ja
docs/pro-studio/P03-animaatioperiaatteet.md. Tarkista ensin, onko P01 yhdistetty mainiin.
Luo worktree haaralle feat/pro-p03-principles, tee tehtävät, aja tarkistukset, avaa PR.
```
