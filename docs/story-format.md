# Tarinakäsikirjoitus (`kilsat_story: 1`)

Korkean tason käsikirjoitusmuoto, jonka Claude, ChatGPT tai ihminen voi kirjoittaa
tarinan ideasta. Työpöytäsovellus kääntää sen tavalliseksi MotionScriptiksi:
hahmot, taustat, kuvakoot, puhe, katseet, eleet, kävelyt, käsiesineet ja tekstitykset.
Tulos on muokattava projekti; mitään ei piiloteta erilliseen ajonaikaiseen malliin.

Koodi: [`crates/motion-script/src/story.rs`](../crates/motion-script/src/story.rs)
(muoto, validointi, kääntäjä, tekoälyohje) ja
[`apps/studio-desktop/src/story_import.rs`](../apps/studio-desktop/src/story_import.rs)
(Tikku-ukkostudion hahmo- ja lavastekatalogi, taustakuvituksen kokoaminen).
Esimerkki: [`examples/story/kahvitauko.yaml`](../examples/story/kahvitauko.yaml).

## Työnkulku sovelluksessa

Vasen paneeli → **Luo animaatio käsikirjoituksesta (Claude / ChatGPT)**:

1. **Kopioi ohje tekoälylle.** Leikepöydälle tulee englanninkielinen ohje, jossa on
   muoto, sallitut arvot, käytettävissä olevat hahmot ja lavasteet sekä esimerkki.
   Liitä se tekoälylle ja kirjoita perään tarinan idea (millä kielellä tahansa).
2. **Liitä vastaus** tekstikenttään. Koko vastaus kelpaa: ensimmäinen
   ```` ```yaml ```` -koodilohko poimitaan automaattisesti.
3. **Luo animaatio.** Nykyinen projekti korvataan; **Kumoa** palauttaa sen.
   Jos käsikirjoituksessa on virheitä, ne näytetään polkuineen ja
   **Kopioi virheet tekoälylle** kopioi korjauspyynnön samaan keskusteluun.

Sen jälkeen: valmistele puhe Puhe-välilehdellä (Kokoro englanniksi, Piper suomeksi),
tarkista ajoitus ja vie video tavalliseen tapaan.

## Muoto

```yaml
kilsat_story: 1
title: Kahvitauko
language: fi                # fi | en | en-gb
captions: true              # valinnainen, oletus true
cast:
  VEIKKO: {holds: kahvikuppi}   # valinnaiset: voice, holds, hand (left|right)
  PASMA: {}
scenes:
  - id: kahvila_aamu        # a-z, 0-9, _
    set: kahvila
    summary: Vapaa kuvaus, ei tulkita.
    positions: {VEIKKO: left, PASMA: far_right}
    beats:
      - do: PASMA
        move_to: right
        gait: jog
      - say: PASMA
        line: Veikko! Palaveri alkaa.
        emotion: surprised
        gesture: point
        reaction: {by: VEIKKO, emotion: sad, hold: 1.2}
      - pause: 1.0
        shot: close_up
        on: VEIKKO
```

| Kenttä | Säännöt |
| --- | --- |
| `cast` | 1–6 hahmoa katalogista. `voice`: Kokoro-esiasetus (`am_michael`) englannille, Piper (`piper_*`) suomelle. `holds`: Tikku-ukkostudion käsiesine; `hand` valitaan vapaasta kädestä, jos sitä ei anneta. |
| `scenes` | 1–12 kohtausta, kussakin 1–48 beatia (yhteensä ≤ 200). `positions` määrää, ketkä ovat lavalla: `far_left`, `left`, `center`, `right`, `far_right`; kaksi hahmoa ei voi aloittaa samasta paikasta. |
| beat | Täsmälleen yksi: `say` (repliikki, vaatii `line`), `do` (toiminta) tai `pause` (0,2–10 s). |
| `say` | `line` ≤ 400 merkkiä, `emotion`, `gesture` (puheen aikana), `look`, `shot`, `on`, `hold` (tauko repliikin jälkeen), `reaction: {by, emotion, gesture, hold}`. Ei `move_to`:ta. |
| `do` | `emotion`, `gesture`, `look`, `face`, `move_to` + `gait`, `hold`, `shot`, `on`. |
| arvot | `emotion`: neutral, happy, sad, surprised, angry · `gesture`: wave, point, nod, shrug · `face`: front, left, right, back · `gait`: walk, jog, run · `shot`: auto, wide, medium, close_up, two_shot, over_shoulder |

Tuntemattomat kentät, hahmot, lavasteet ja arvot hylätään (ei arvata, ei ohiteta).

## Kääntäminen

- **Yksi MotionScript-kohtaus per tarinakohtaus**, 1920×1080, 24 fps. Tausta kopioidaan
  lavasteen kuvituksesta kohtauksen omiksi tasoiksi; lavalla olemattomat hahmot piilotetaan.
- **Ajoitus:** beatit peräkkäin; alussa 0,5 s, beattien välissä 0,3 s, lopussa 0,8 s.
  Repliikin kesto on **arvio** (max(0,42 s/sana, 0,068 s/merkki, 1 s)), ellei kutsuja anna
  mitattuja kestoja (`compile_story(…, Some(&kestot))`). Beatien alut pyöristetään ruutuihin.
- **Kuvat ja leikkaukset:** jokainen otos on kamera-avainpari; leikkaus on kova (edellinen
  rajaus pysyy 1 ms ennen leikkausruutua). Automaattisäännöt: kohtauksen alku `wide`,
  repliikki `medium` puhujaan, voimakas tunne (angry, surprised, sad) `close_up`,
  saman puhujan jatkorepliikki ei leikkaa, kävely `wide`, `reaction` leikkaa kuulijaan.
  Yli 1,5 s otoksissa on hidas 2,5 %:n zoomaus. Rajaus pysyy maalatun taustan sisällä.
- **Näyttely:** kuulijat kääntävät katseensa puhujaan 0,2 s repliikin alun jälkeen;
  puhuja katsoo `look`-hahmoa, edellistä puhujaa tai lähintä hahmoa. Ele alkaa 0,15 s
  repliikin jälkeen ja kestää 0,8–1,6 s. Kävely kääntää hahmon profiiliin kulkusuuntaan ja
  palauttaa aiemman suunnan perillä.
- **Ääni:** hahmo saa äänen kielen mukaan (Kokoro en-us/en-gb tai Piper fi-fi). Suomessa
  kaikilla on oletuksena `piper_harri`, ellei `voice` kerro muuta. Tekstitykset saavat
  hahmon tunnusvärin.

## Rajoitukset

- Ajoitus on tekstiarvio. Työpöytäsovellus ei vielä uudelleenajoita tarinaa oikean puheen
  kestoilla, vaikka kääntäjä tukee annettuja kestoja. Jos oikea puhe on arviota pidempi,
  saman hahmon peräkkäiset repliikit voivat mennä päällekkäin, ja koko kohtauksen äänen
  valmistelu hylkää ne.
- Hahmot ja lavasteet tulevat vain upotetusta Tikku-ukkostudiosta (6 hahmoa, 4 lavastetta).
- Eleitä on neljä ja tunteita viisi; esineen poimiminen tai laskeminen kesken kohtauksen,
  istuminen, kosketukset ja kameran ajot puuttuvat.
- Laatu (rytmi, rajaukset, huulisynkka, ääni) vaatii ihmisen arvion; testit osoittavat vain
  determinismin, validoinnin ja rajat.
