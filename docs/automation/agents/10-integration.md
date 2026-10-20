# Agentti 10 — yhteinen sopimus ja toteutusjärjestys

Tila: **suunnitelma**, ei automaattisen animaatiotuotannon toteutus tai
hyväksyntä. Tämä dokumentti yhdistää kymmenen agentin suunnittelutyön nykyiseen
Kilsat Studion putkeen. Koonti: [automaation suunnitelma](../README.md).
Tuotteen kokonaiseteneminen: [master-progress](../../master-progress.md).

## Yhteinen rajaus

Ensimmäinen sovittava profiili on alkuperäinen tyylitelty 2D-animaatio:
30–60 sekuntia, enintään kaksi valmista hahmoa, kolme **tarinakohtausta** ja
16 lopullista otosta, reaktiot mukaan lukien. Nykyinen `expand_shots` tekee
jokaisesta otoksesta teknisen `Scene`-rakenteen; tarinakohtausten rajaa ei
saa soveltaa vahingossa näihin laajennettuihin sceneihin. Lyhyempi tapaus ei
todista 60 sekunnin enimmäistapauksen valmistumista.

Pixar ja Disney kuvaavat käyttäjän laatutoivetta. Tämä työ ei osoita vastaavaa
laatua, tuota niiden hahmoja tai lupaa mielivaltaisen 3D-tuotannon valmistuvan
300 sekunnissa. Valmiiden hahmojen, taustojen, äänten ja oikeuksien valmistelu
on näkyvä ennakkoehto; sitä ei piiloteta automaation väitetyksi tulokseksi.

## Versioidut siirtosopimukset

Kaikki alla olevat `v1`-suunnittelutyypit ovat **ehdotuksia**. Niitä ei tällä
dokumentilla lisätä MotionScriptin tiukkaan YAML-skeemaan.

| Sopimus | Omistaja ja vastuu | Seuraava kuluttaja |
| --- | --- | --- |
| `ScriptIntent v1` | Käsikirjoitus: muuttumaton dialogi, lähdetiiviste, UTF-8-tavuvälit, vakaat beat-/hahmo-/tarinakohtaus-ID:t, lukitukset ja ratkaisemattomat kohdat | Aineistovalinta ja ohjaus |
| `AssetManifest v1` | Aineistot: täsmälliset revisiot, SHA-256, paikalliset viitteet, rigikyvykkyydet, tavurajat ja oikeuksien tarkistusnäyttö | Kaikki suunnittelijat ja viennin esitarkistus |
| `AudioPlan v1` | Puhe: todellinen PCM, näytemäärät, kieli-/ääni-/provider-identiteetti, johdannaisavaimet ja ajoituksen alkuperä | Ohjauksen lopullinen aikataulu ja suunliikkeet |
| `DirectionPlan v1` | Ohjaus: beatien tarkoitus, yhteinen aikataulu, akseli, toiminnan jatkuvuus ja käyttäjän lukitukset | Kamera ja näyttely |
| `ShotPlan v1` | Kamera: enintään 16 lopullista otosta, yksi kameraomistaja, rajaus sekä puheen ja insertin varaukset | Nykyisen `ShotList`-rakenteen adapteri |
| `ActingPlan v1` | Näyttely: kanava-/nivelomistus, poseeraus, ele, katse, ilme ja kontaktivaatimukset | Nykyiset tapahtumat, nivel-/katse-/klippiraidat |
| Päätös- ja ajon raportti | Integraatio: sopimus-/sääntö-/asset-/build-identiteetit, vaiheajat, diagnoosit, todelliset ulostulot ja erilliset hyväksyntätilat | Editorin korjauspolku, CI ja Mac-hyväksyntä |

Kanoninen ajoitus käyttää yhteisen 24 kHz:n äänikellon kokonaisnäytteitä ja
puoliavoimia aikavälejä. Ruudun indeksi on näistä johdettu näyttely-/renderöinnin
näytteistys, ei toinen itsenäisesti kertyvä kello. `ActingPlan` voi säilyttää
ruutukvantisoidun eleen, mutta puheankkuri pysyy alkuperäisenä näyteindeksinä.
MotionScript-sekunteihin muuntaminen tehdään adapterin rajalla ja tarkistetaan
takaisin yhteistä aikataulua vasten. Äänikeston muutos invalidioi leikkauksen,
näyttelyn, tekstityksen ja renderin riippuvat välimuistit.

Jokainen suunnittelija vastaanottaa saman lähteen, profiilin, säännöstön ja
aineistomanifestin identiteetin. Puuttuva tai tuntematon versio, muuttunut lähde,
ei-äärellinen luku tai tuntematon viite palauttaa korjattavan virheen.
Suunnittelutyyppiä ei hyväksytä vain siksi, että se on syntaktisesti JSONia.

## Sääntöjen ratkaisu ja liitos nykykoodiin

1. Kovat ajoitus-, kyvykkyys-, resurssi- ja oikeusrajat ovat portteja. Niiden
   vastainen käyttäjän lukituskin tarvitsee korjauksen, ei hiljaista ohitusta.
2. Kelvollinen eksplisiittinen käyttäjän tieto voittaa hyväksytyn tuotantopaketin
   oletuksen; oletus voittaa valinnaisen esteettisen ehdotuksen. Ristiriitaiset
   pakolliset valinnat raportoidaan lähdeviitteillä.
3. Suunnitteluehdokkaiden tasajärjestys on vakaa: lähdebeatin järjestys,
   sääntö-ID ja ehdokas-ID. Siemenellä valitaan vain hyväksyttyjä samanarvoisia
   variantteja; se ei ratkaise pronominin merkitystä tai lisenssipuuttetta.
4. Nykyisen `Script::expand_rules`-funktion nimijärjestys, alkuperäisten
   tapahtumien laukaisimet ja rekursion puuttuminen säilyvät. Uutta ajonaikaista
   sääntötulkkia ei tuoda rinnalle. Suunnittelijan prioriteetti ratkaistaan ennen
   nykyisen lähteen tuottamista, eikä nykyisen `Rules`-kartan semantiikkaa muuteta.
5. Shot-adapteri käyttää `ShotList`, `ShotSpec`, `ShotLine`, `expand_shots` ja
   `ShotExpansion.plan` -rakenteita. Se ei jätä pohjascriptiin toista kopiota
   tarinakohtauksesta. Shot-kamera on kyseisen otoksen ainoa kameraomistaja.
6. Lopullinen lähde kulkee nykyisen `parse`/`Script::validate`-polun kautta
   samaan kohtausarviointiin ja esikatselu-/vientiputkeen. Tuntematon toiminto
   palauttaa `Unsupported`-diagnoosin, eikä katoa onnistumiseksi merkitystä työstä.

Tarkastettu pohja: `main`-commit `7bb5e21331a672b15be5b2227c93ccc8171e8fa7`.
`motion-script` sisältää jo `Composition`-nivel-/katseraidat ja `RigClip`-
toiminnot. `motion-jobs` vie julkisesti `MemoryBudget`, `process_budget` ja
`Graph`-tyypit; uuden orkestroijan tulee käyttää niitä eikä julistaa puuttuvaksi
koko nykyistä muistikirjanpitoa tai riippuvuuksien ajoitusta. Näiden olemassaolo
ei yksin todista koko prosessin kattavuutta tai viiden minuutin suorituskykyä.

## Riippuvuuksien mukainen toteutusjono

Kaikki alla olevat uuden automaation tehtävät ovat avoinna. Suunnitteludokumentti
tai sen CI-tarkistuksen onnistuminen ei merkitse vastaavaa tuoteominaisuutta valmiiksi.

| Järjestys | Konkreettinen muutos ja paikka | Riippuvuus ja hyväksyntä |
| --- | --- | --- |
| `INT-01` | Versioidut intentti-/suunnitelmasopimukset ja adapterin diagnoosit `motion-script`-vastuuseen; rajattu suomenkielinen tuonti editoriin | Lähdeviitteet ja dialogi säilyvät; epäselvä puhuja pysäyttää tulkinnan. Ei kilpailevaa YAML-jäsentäjää. |
| `INT-02` | Aino–Rane-tyylipaketti, rigikyvykkyydet ja versioitu aineistomanifesti nykyisen projektin aineistorajalle | INT-01; jokaisella tiedostolla oikea sisältö ja tarkistusnäyttö, ei tekaistuja hash- tai hyväksyntätietoja. |
| `INT-03` | `AudioPlan` nykyisten `motion-tts`/`motion-voice`/`motion-audio`-rajojen päälle; hyväksytty suomenkielinen tuontipuhe ensimmäiseksi | INT-01/02; nykyinen `SpeechRequest::validate` sallii vain `en-us`/`en-gb`. Suomen TTS vaatii erillisen toteutuksen, oikeudet ja todellisen kielilaadun varmennuksen. |
| `INT-04` | Ohjaus- ja kamera-adapteri nykyiseen `shots.rs`-kirjastoon | INT-03:n mitatut kestot; 16 lopullisen otoksen raja, akseli, luettava 5 s oikea Kilsat-insertti ja yksi kameraomistaja. |
| `INT-05` | Tarkoituspohjainen näyttelyvalinta nykyisiin klippeihin, nivel-/katseraitoihin ja suun variantteihin | INT-02/03/04; kanava-/nivelristiriidat ja kontaktin mahdottomuus raportoidaan. Tuettu rigi, haku ja vienti tuottavat saman asennon. |
| `INT-06` | Työ-DAG, sisältöavaimet, peruutus ja vaiheiden resurssivaraukset nykyiseen `motion-jobs`-putkeen | INT-01…05; vika/peruutus vapauttaa varaukset, vain riippuvat tulokset vanhenevat ja UI pysyy responsiivisena. |
| `INT-07` | Rajattu streaming-vientipolku `motion-export`-vastuuseen, ennen Full HD -profiilin avaamista | Yhteinen muistibudjetti, levy-/encoderin vastapaine, peruutus, virhetestit ja todellinen 60 s H.264/AAC-ulostulo; ei pelkkää rajavakion nostoa. |
| `INT-08` | Editorin lähde → tarkistus → valmisprofiili → ajo → korjaus -polku `studio-desktop`-sovellukseen | INT-01…06; Undo, vanhentuneen luonnoksen hylkäys, näppäimistöpolku ja fyysisen Macin koe. |
| `INT-09` | Koneelliset portit ja riippumaton äänellisen pilotin laatukatselmus | INT-02…08; oikea kuvan/äänen ulostulo, havaittava vitsin rytmi, katse/ele/kontakti ja luettava sovelluskuva. |
| `INT-10` | Nimetyllä Macilla toistettu enimmäistapauksen end-to-end-mittaus ja julkaisuportti | Kaikki edelliset; suorituskyky, taiteellinen arvio, laitehavainnot ja oikeudet ovat erilliset hyväksyntätulokset. |

Nykyinen rasterivienti rajaa kumulatiiviset pikselit miljardiin ja välituotetavuja
256 MiB:iin. 1920×1080×24×60 = 2 985 984 000 pikseliä ylittää ensimmäisen rajan;
myös 30 s Full HD ylittää sen. Tätä ei voi merkitä valmiiksi Full HD -profiiliksi.
Erikseen nimettävä 960×540/24 fps -referenssiprofiili alittaa pikselirajan 60 s
tapauksessa (746 496 000), mutta sen läpäisy ei takaa tavubudjettia, laatua,
suorituskykyä tai Full HD -kelpoisuutta. Resoluutio ei vaihdu automaattisesti.
Tuotannon GPU-esikatselu/vienti ja täysi CPU/GPU-pariteetti säilyvät omina töinään.

## Yhteinen 300 sekunnin budjetti

| Vaihe | Ehdotettu enimmäisosuus |
| --- | ---: |
| Tuonti ja intentin validointi | 5 s |
| Ohjaus, kamera ja ristiriitojen ratkaisu | 10 s |
| Paikallisten aineistojen tarkistus | 10 s |
| Todellisen puheen tuotto tai tuonti | 60 s |
| Äänen ajoitus ja suunliikeaineisto | 20 s |
| Näyttelyn valmistelu ja alennus | 20 s |
| Renderöinti | 145 s |
| Enkoodaus ja lopputiedoston tarkistus | 20 s |
| Kokonaisuuden varaus | 10 s |
| **Yhteensä** | **300 s** |

Ohjauksen 5 s ja kameran 2 s ovat 10 s suunnitteluosuuden sisäisiä kattoja;
näyttelyn ehdotettu 5 s suunnittelutyö sisältyy sen 20 s osuuteen. Osabudjetteja
ei lasketa uudelleen kokonaisuuden päälle. Mahdollinen rinnakkaisuus käyttää
vain samoja hyväksyttyjä riippuvuuksia ja resursseja; sitä ei oleteta nopeudeksi
ilman mittausta. 60 s / 24 fps = 1440 ruutua; 145 s renderiosuus edellyttää
keskimäärin noin 9,94 valmista ruutua sekunnissa, lisäksi muut vaiheet.

300 s mitataan valmiin syötteen käynnistämisestä oikean videotiedoston
tarkistettuun valmistumiseen, jonotus mukaan lukien. Puuttuva assetti tai
selvittämätön käsikirjoitus estää valmiin profiilin valinnan; ajon aikana syntynyt
odotus tai virhe ei pysäytä mittaria ja muutu onnistuneeksi tulokseksi.
Ylitys näkyy `deadline_exceeded`-tilana; puhetta, laatua tai tarkistusta ei
jätetä pois määräajan täyttämiseksi. Lämmin/kylmä välimuisti ja ensimmäinen ajo
raportoidaan erikseen todellisen buildin ja koneen tunnisteilla.

Tämän PR:n suunnitelmatarkistin ja sen CI voivat osoittaa dokumenttien,
esimerkkiaikataulun ja budjettisopimuksen yhtenäisyyttä. Ne eivät aja yllä olevia
uusia tuotantovaiheita, todista suomenkielistä puhetta tai anna ammattitason,
fyysisen Macin, lopullisten lisenssien tai 300 sekunnin hyväksyntää.
