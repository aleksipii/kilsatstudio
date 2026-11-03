# Lavastus, vuorokaudenajat ja suomenkieliset hahmoäänet

Koodi: [`studio-desktop/src/set_dressing.rs`](../apps/studio-desktop/src/set_dressing.rs),
[`motion-art/src/time_of_day.rs`](../crates/motion-art/src/time_of_day.rs) ja
[`motion-tts/src/finnish_voices.rs`](../crates/motion-tts/src/finnish_voices.rs).

## Lavastuseditori

Kuvitus-tilan paneelissa on osio **Lavastus ja vuorokaudenaika**:

- **Esine:** Tikku-ukkostudion 33 kuvitusesinettä (autot, puut, penkit, talot,
  kyltit, automaatit ja muut), haettavissa nimellä. Käsiesineitä, kuten puhelinta
  ja kahvikuppia, ei listata, koska ne ovat rigin osia ja kulkevat hahmon kädessä
  `holds`-kentällä.
- **Korkeus** pikseleinä sekä **Hahmojen takana** tai **Hahmojen edessä**.
- **Lisää keskelle lattiaa** tai **Sijoita napsauttamalla**: esineen alareunan keskikohta asettuu
  napsautuskohtaan.
- **Lavastustila:** näyttämön napsautus valitsee esineen, ja veto siirtää sitä.
  Valittu esine näkyy keltaisella kehyksellä.
- Valitulle esineelle: Pienennä ja Suurenna 10 % kerrallaan (alareuna pysyy
  lattialla), Peilaa, siirto hahmojen eteen tai taakse sekä Poista.

Lavastettu esine on tavallista kuvitusta. Sen tasot saavat yhteisen tunnuksen
`dress<N>_…` ja esineen nimen, joten myös muut kuvitustyökalut toimivat niihin.
Jokainen valmis toiminto, kuten koko veto, lisäys tai poisto, on yksi
kumottava kuvitusmuutos. Vedon aikana esikatselu päivittyy, mutta historiaan
tulee vain lopputulos. Kaikki tulokset validoidaan kuvitusdokumentin omilla säännöillä.

## Vuorokaudenajat

Kohtauksen vuorokaudenaika on valmis valoyhdistelmä:

| Aika | Sävy | Vinjetti |
| --- | --- | --- |
| Päivä (`day`) | ei valoja | – |
| Aamu (`morning`) | lämmin vaalea (255, 228, 196), 28 % | kevyt |
| Ilta (`evening`) | oranssi (255, 172, 118), 42 % | keskivahva |
| Yö (`night`) | tumman sininen (58, 78, 140), 74 % | vahva |

Valot ovat koko kuvan kertovaa sävytystä (`Ambient`) ja vinjetti. Ne piirretään
kuvituksen ja hahmojen päälle, joten lavaste ja hahmot sävyttyvät samalla
tavalla. Ne ovat tavallisia valoja tunnuksilla `tod/<aika>/<osa>/<kohtaus>`.
Ajan vaihto korvaa vain nämä valot, ja kohtauksen muut valot säilyvät.

Tarinassa: `time: day | morning | evening | night` kohtauksessa (oletus
`day`). Tekoälyohje kertoo kentästä. Työpöydällä aika valitaan lavastusosiosta.

## Suomenkieliset hahmoäänet

Piperin julkisessa äänikirjastossa on suomeksi yksi puhuja, Harri. Uudet äänet
ovat sen **sävelkorkeusmuunnelmia, eivät eri puhujia**:

| Tunnus | Muutos | Tikku-ukkostudion hahmo |
| --- | --- | --- |
| `piper_harri` | – | Veikko |
| `piper_harri_jykea` | −5 sävelaskelta | Noutaja |
| `piper_harri_matala` | −3 | Päsmä |
| `piper_harri_korkea` | +3 | Kiire, Sisu |
| `piper_harri_kirkas` | +5 | Kipinä |

Toteutus: Piper syntetisoi puheen pidempänä tai lyhyempänä (`length_scale`
kerrotaan sävelkertoimella), ja tulos uudelleennäytteistetään 24 kHz:iin
kertoimen verran nopeammin tai hitaammin. Kesto pysyy samana, mutta
sävelkorkeus ja formantit siirtyvät yhdessä, jolloin ääni kuulostaa
isommalta tai pienemmältä puhujalta. Muunnelma käyttää samaa mallia ja
profiilia kuin `piper_harri`, eikä uusia malleja ladata. Välimuistin avain
sisältää sävelkertoimen. Äänieditori listaa äänet, ja tarina-asetuksessa
jokaisella hahmolla on oma oletusäänensä.

## Tarkistukset ja rajoitukset

- Testit kattavat esineiden sijoituksen mittoihin, siirron, koon (alareuna
  pysyy paikallaan), peilauksen, kerroksen ja poiston. Lisäksi ne kattavat
  kaikkien esineiden validin sijoituksen, luettelon vastaavuuden paketin
  manifestin kanssa, vuorokaudenaikojen vaihdon ja muiden valojen säilymisen,
  tarinan `time`-kentän ja sen valot, äänitaulukon puolisävelaskeleet sekä
  sävelkorkeuden muutoksen mitattuna testisignaalista (220 Hz → 262 / 165 Hz).
- Iltavalaistu ja lavastettu kohtaus on katsottu kuvana: penkki on hahmon
  takana, taksi edessä, ja sävy kattaa koko kuvan.
- **Oikeaa suomenkielistä Piper-puhetta ei syntetisoitu**, koska tässä
  koneessa ei ole Piperiä eikä Harrin mallia, eikä niitä ladattu ilman lupaa.
  Muunnelmien ymmärrettävyys ja luonnollisuus on kuunneltava, kun malli on
  asennettu. Etenkin ±5 sävelaskeleen muutoksessa voi kuulua "orava-" tai
  "jättiläisääni".
- Työpöydän lavastusta ei ole käytetty käsin hiirellä, eikä sitä ole testattu
  fyysisellä Macilla.
- Vuorokaudenajat eivät lisää ikkunoihin tai lamppuihin omia valoja. Ne voi
  lisätä valotyökalulla.
