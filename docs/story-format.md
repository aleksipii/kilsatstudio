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
3. **Luo animaatio oikealla puheella.** Jokainen repliikki tuotetaan hahmon äänellä
   (Kokoro englanniksi, Piper suomeksi) taustatyönä, WAV puretaan ja tarkistetaan,
   tallennetaan sisältötiivisteellä projektikansion `assets/auto-direction/`-kansioon ja
   tarina ajoitetaan mitattujen kestojen mukaan. Jokainen `speak`-tapahtuma soittaa
   omaa WAViaan, joten esikatselu ja vienti käyttävät samaa ääntä. Työ on rajattu
   (30 min), peruttavissa ja vanhentunut tulos hylätään. Projektikansio on annettava.
   **Luonnos ilman puhetta** tekee saman heti tekstiarviolla.
   Nykyinen projekti korvataan; **Kumoa** palauttaa sen. Virheet näytetään polkuineen,
   ja **Kopioi virheet tekoälylle** kopioi korjauspyynnön samaan keskusteluun.
4. **Leikkaustarkastus** näkyy luonnin jälkeen (ks. alla).

Ohje ja käännös tarjoavat Tikku-ukkostudion lisäksi **avoimen projektin omat
rigatut hahmot ja kuvitetut kohtaukset** (kohtaus → lavaste, tunnus pienaakkosin,
`oma_`-etuliite jos se peittäisi paketin lavasteen). Kopioi ohje uudelleen, kun lisäät
omia hahmoja tai taustoja.

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
| `scenes` | 1–12 kohtausta, kussakin 1–48 beatia (yhteensä ≤ 200). `positions` määrää, ketkä ovat lavalla: `far_left`, `left`, `center`, `right`, `far_right` sekä ruudun ulkopuoliset `offscreen_left` ja `offscreen_right`; kaksi hahmoa ei voi aloittaa samasta ruudun sisäisestä paikasta, mutta ruudun ulkopuolella saa odottaa useampi. |
| beat | Täsmälleen yksi: `say` (repliikki, vaatii `line`), `do` (toiminta) tai `pause` (0,2–10 s). |
| `say` | `line` ≤ 400 merkkiä, `emotion`, `gesture` (puheen aikana), `look`, `shot`, `on`, `hold` (tauko repliikin jälkeen), `reaction: {by, emotion, gesture, hold}`, `move_to` + `gait` (kävely alkaa repliikin kanssa, "walk and talk"). |
| `do` | `emotion`, `gesture`, `look`, `face`, `move_to` + `gait`, `holds` (+ `hand`), `pose`, `hold`, `shot`, `camera`, `on`. |
| `holds` | Käsiesine poimitaan perille saavuttaessa; `none` laskee sen. Ilman `hand`-kenttää valitaan vapaa käsi (vasen ensin, jotta oikea jää eleille). Tila alkaa joka kohtauksessa castin `holds`-arvosta. |
| `move_to` | Paikka (myös `offscreen_left`/`offscreen_right`: sisääntulo ja poistuminen) tai lavalla oleva hahmo: kävelijä pysähtyy 16 % ruudun leveydestä hänen eteensä tulosuunnan puolelle ja katsoo häntä. |
| `path` | 2–6 reittipistettä (paikkoja tai hahmoja), jotka kävellään yhtenä matkana: kiihdytys alussa, jarrutus lopussa, ei pysähdyksiä välissä. Ei yhdessä `move_to`:n tai `follow`:n kanssa. |
| `follow` | Hahmo kävelee edellisessä beatissa liikkuneen hahmon perässä: lähtee 0,4 s myöhemmin, samalla askellajilla (ellei `gait` muuta kerro) ja pysähtyy 12 % ruudun leveydestä hänen taakseen. Ei ohita johtajaa. Vain `do`-beateissa. |
| `depth` | `near`, `mid` (oletus) tai `far`: hahmo kävelee kameraa kohti tai siitä poispäin, kasvaa (×1,25) tai pienenee (×0,7) ja laskee tai nousee ruudussa. Pelkkä `depth` kävelee suoraan kohti kameraa (kasvot edellä) tai poispäin (selkä edellä); `move_to`:n kanssa vinosti. Piirtojärjestys ei muutu. |
| `pace` | Nopeuskerroin 0,5–1,5 mille tahansa askellajille. Askelrytmi ei muutu, joten hyvin hitaalla tai nopealla tahdilla jalat voivat liukua hieman. |
| `look_back` | `true` `move_to`:n kanssa: hahmo pysähtyy puolivälissä, katsoo taakseen 0,9 s ja jatkaa. |
| väistäminen | Kun kävelijä kulkee seisovan hahmon kohdalta (samalla syvyydellä, ei istumassa eikä kävelyn kohde), tämä astuu 45 px taaemmas ja palaa, kun kävelijä on ohittanut. Väistöt näkyvät käännöksen huomautuksissa. |
| `gait` | Askellaji `move_to`:n, `path`:n, `follow`:n tai `depth`:n kanssa. `carry` on hidas, kumara kantaminen; se valitaan automaattisesti, kun hahmolla on esine kummassakin kädessä. Kävelyt kiihdyttävät ja jarruttavat (`ease: in_out`); `backwards` kulkee selkä edellä (kasvot poispäin kulkusuunnasta); `leap` on yksi hyppy, enintään 35 % ruudun leveydestä. Nopeudet px/s: stroll 180, sneak 150, tiptoe 170, limp 150, backwards 150, walk 260, jog 450, leap 520, run 700, sprint 950, carry 160. Hahmon tunne näkyy askelissa: surullinen laahustaa (pienet kädenheilahdukset, pää alhaalla), vihainen tömistelee (raskas pomppu, jäykät kädet) ja iloinen hypähtelee. |
| `pose` | `sit` lisää lavasteeseen toimistotuolin hahmon alle (istuimen korkeus sivu- tai etukuvan mukaan); `crouch` kyykky, `kneel` polvillaan, `lie` makaa lattialla, `fall` kaatuu nopeasti (0,35 s) makaamaan; `stand` nousee. Hahmo voi kävellä vain seisaaltaan: `pose: stand` samassa beatissa `move_to`:n kanssa nousee ensin. |
| `camera` | `static`, `push_in`, `pull_out` (zoomi 30 %), `pan_left`, `pan_right` (12 % näkyvästä leveydestä kumpaankin suuntaan, zoomi vähintään 1,2), `shake` (0,5 s vaimeneva tärähdys). Liike aloittaa aina oman otoksensa. |
| `cast.*.speed` | Puhenopeus 0,7–1,4. Hahmoilla on oletusnopeudet (esim. Kiire 1,18, Noutaja 0,88); suomessa ne erottavat hahmot, koska Piperissa on yksi ääni. |
| `principles` | Animaatioperiaatteet: `cast.*.principles` hahmolle ja beatin `principles` sen eleelle. Esiasetus (`subtle`, `cartoon`, `snappy`) tai lohko; ks. [animation-principles.md](animation-principles.md). Beatin `principles` vaatii `gesture`-kentän. |
| arvot | `emotion`: neutral, happy, sad, surprised, angry · `gesture`: wave, point, nod, shrug, shake_head, facepalm, arms_crossed, thumbs_up, bow, jump, clap, think, stumble · `face`: front, left, right, back · `gait`: walk, jog, run, stroll, sneak, tiptoe, limp, sprint, backwards, leap, carry · `pose`: stand, sit, crouch, kneel, lie, fall · `depth`: near, mid, far · `shot`: auto, wide, medium, close_up, two_shot, over_shoulder |

Tuntemattomat kentät, hahmot, lavasteet ja arvot hylätään (ei arvata, ei ohiteta).

## Kääntäminen

- **Yksi MotionScript-kohtaus per tarinakohtaus**, 1920×1080, 24 fps. Tausta kopioidaan
  lavasteen kuvituksesta kohtauksen omiksi tasoiksi; lavalla olemattomat hahmot piilotetaan.
- **Ajoitus:** beatit peräkkäin; alussa 0,5 s, beattien välissä 0,3 s, lopussa 0,8 s.
  Repliikin kesto on mitattu oikeasta puheesta (`MeasuredLine`) tai luonnoksessa arvio
  (max(0,42 s/sana, 0,068 s/merkki, 1 s)). Beatien alut pyöristetään ruutuihin.
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

## Leikkaustarkastus

Luonnin jälkeen sovellus näyttää mitattavat leikkausongelmat (`story::ReviewNote`):
puhuja kuvan ulkopuolella repliikin neljänneksillä, alle 0,75 s välähdysotokset,
yli 10 s liikkumattomat otokset (paitsi `camera: static` tai liikkuva kohde), yli
18 merkkiä/s puhe ja yli 4 s hiljaisuus repliikkien välissä. Tarkastus ei arvioi
komiikkaa, näyttelyä tai huulisynkan laatua.

## MotionScript-lisäykset

Tarinakääntäjä käyttää kahta uutta tavallista MotionScript-tapahtumaa, joita voi
myös muokata editorissa:

- `{type: hold, actor, at, hand: left|right, item?}`: käsi pitää luettelon esinettä
  hetkestä `at` alkaen, tai ei mitään, jos `item` puuttuu. Arvioija valmistelee rigin
  jokaiselle käsitilalle ja valitsee sen ajan perusteella, joten haku on tarkka.
  Vaatii oman rigin; varattu käsi tai tuntematon esine on validointivirhe.
- `{type: pose, actor, at, value: stand|sit|crouch|kneel|lie}`: asento vaihtuu
  pehmeästi (sit/stand 0,4 s, crouch 0,3 s, kneel 0,5 s, lie 0,9 s) siitä asennosta,
  jossa hahmo sillä hetkellä on, ja kestää seuraavaan pose-tapahtumaan. Vaatii rigin,
  jossa on root-, thigh- ja shin-nivelet. Jalkojen kulmat on mitoitettu
  tikku-ukkostudion mittasuhteille (reisi 70, sääri 62).
- `{type: move, …, gait?, ease?}`: `gait` on jokin yllä luetelluista askellajeista ja
  `ease` nopeusprofiili `linear` (oletus), `in`, `out` tai `in_out` (kiihdytys ja
  jarrutus 25 % kestosta, tasainen vauhti välissä). Kiihdyttävän liikkeen askelrytmi
  seuraa kuljettua matkaa, joten jalat eivät liu'u. Liikkeen kohde saa olla enintään
  yhden ruudun leveyden tai korkeuden verran kuvan ulkopuolella.

Uudet eleet toimivat sekä omilla rigeillä (ennakointi–ylilyönti–pito–palautus) että
alkuperäisellä seitsennivelrigillä.

## Rajoitukset

- Uusien askellajien ja asentojen ulkoasu on tarkistettu renderöidyistä ruuduista, mutta
  niitä ei ole säädetty animaattorin kanssa. Kaatumisen nopeassa siirtymässä jalat
  painuvat hetkeksi hieman lattian alle. Syvyysliike ei muuta piirtojärjestystä, joten
  taaempana kulkeva hahmo voi piirtyä edessä seisovan päälle; sama koskee väistäjää.
- Suomessa on yksi Piper-ääni (Harri); hahmot erottuvat vain puhenopeudella, kunnes
  käyttäjä asentaa muita suomenkielisiä Piper-malleja.
- Tunteita on viisi, koska ne vaativat rigiin piirretyt ilmevariantit.
- Tuoli on aina toimistotuoli etunäkymästä piirrettynä; sivukuvassa se on tyylitelty.
- Käsien ja istumisen tila alkaa jokaisessa kohtauksessa alusta (castin `holds`).
- Ajastettu esine ei voi olla `composition.screens`-näytön kohde (näyttö vaatii castin
  `holds`-esineen).
- Laatu (rytmi, rajaukset, huulisynkka, ääni) vaatii ihmisen arvion; testit osoittavat vain
  determinismin, validoinnin ja rajat.
