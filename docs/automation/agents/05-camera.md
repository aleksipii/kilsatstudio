# Agentti 05 — kamera, sommittelu, valo ja luettavuus

Tila: **suunnitelma**, ei toteutettu automaattiohjaaja eikä mitattu laatulupaus.
Liittyy [master-progressiin](../../master-progress.md). Tavoite on omaperäinen,
viimeistelty tyylitelty animaatio. Pixar ja Disney ovat käyttäjän laatutoiveen
vertailukohtia; niiden hahmoja, kuvia tai omaisuutta ei kopioida, eikä vastaavaa
tuotantolaatua luvata nykyisestä 2D-putkesta.

## Nykyinen toteutus ja liitoskohta

Tarkastettu lähdepohja: `7bb5e21331a672b15be5b2227c93ccc8171e8fa7`.

| Nykyinen tiedosto / symboli | Varmennettu toiminta | Vielä suunniteltava |
| --- | --- | --- |
| `crates/motion-script/src/shots.rs`: `ShotList`, `ShotSpec`, `expand_shots` | Puhdas, satunnaisuudeton laajennus tavallisiksi kohtauksiksi; kameran avaimet ja näyttelijöiden paikat | `DirectionPlan v1` → validoitu otoslista, rajattu automaattinen sommittelu |
| `ShotPreset` | `establishing`, `medium`, `close_up`, `over_the_shoulder`, `reaction_hold`; zoomit 1, 1.5, 2.4, 1.8 ja 2 | Kuvan sisältöön perustuva rajaus sekä kasvojen, käden ja rekvisiitan turvarajat |
| `CutRules`, `ShotLine`, `PlannedShot` | Annettu puheen kesto määrää leikkauspisteet; oletukset lead-in 0 s, tail 0.5 s, reaktio 1.5 s; raportoi absoluuttiset leikkaus- ja puherajat | Mitatun äänen kytkentä käsikirjoituksen kokonaisaikaan ja dramaturgisiin taukoihin |
| `Composition.camera`, `CameraKey` | Olemassa oleva kamera käyttää `at`, `x`, `y`, `zoom`; shot-laajennin ottaa kameran omistajuuden | Rajausten visuaalinen tarkistus kaikilla olennaisilla ajanhetkillä |
| `Project.captions`, `CaptionStyle`, `CaptionMode` | Yhteisen SVG-polun tekstit; puhujan väri, kirjasinkoko, reunus ja marginaalit; Show ilman tyyliä on virhe | Automaattinen tekstin ja kasvojen/puhelimen päällekkäisyyden ratkaisu |
| `Composition` | Taustavärit, kerrokset, avaimet, kamerat; ei fyysisen valon parametreja | Tämän profiilin valaisutunnelma tehdään hyväksytyistä esileivotuista tausta-/taideasseteista; mahdollinen 3D-valaistus on erillinen kehitystyö |

`examples/night-room-reference/README.md` kuvaa nykyistä omaperäistä flat cut-out
-referenssiä ja myös vanhoja TODO-kohtia. Sen placeholder-puhelinta ja
äänimerkkejä ei hyväksytä valmiiksi sovellusnäkymäksi tai puheeksi. Ajantasainen
toimintatuki tarkistetaan lähdekoodista, ei pelkästä referenssin TODO-listasta.

## Ehdotettu rajapinta

Syöte `DirectionPlan v1` sisältää vakaat beat- ja tarinakohtaus-ID:t,
mitatut puheen alkamis-/päättymisnäytteet, kuvasuhteen, hahmojen ja olennaisen
rekvisiitan rajauslaatikot, tarinatilan akselin, katsekohteet, tyylipaketin
sisältötiivisteen sekä mahdolliset käyttäjän lukitukset. Epävarma kesto merkitään
arvioksi; sitä ei päästetä lopullisen videon puheajoituksen lähteeksi.

Tuloste `ShotPlan v1` sisältää jokaiselle otokselle:

- `id`, `story_scene_id`, `beat_ids`, `start_sample`, `end_sample` ja `subject_ids`;
- `preset`, `target`, `offset`, `side`, `push` sekä näyttelijöiden `place`-tiedot;
- varatut alueet `faces`, `hands`, `captions`, `app_insert` ja perusteltu `rule_trace`;
- tyylipaketin identiteetin, valaistun tausta-assetin identiteetin ja diagnoosit.

Nämä ovat **ehdotettuja suunnittelukenttiä**, eivät uusia MotionScript-kenttiä.
Adapteri alentaa suunnitelman nykyisiin `ShotSpec`-, `ShotLine`- ja
`Composition`-rakenteisiin. Näyteajat muunnetaan sekunneiksi yhdessä kohdassa,
nykyisen 24 kHz:n yhteisen äänikellon mukaan, ja laajennuksen `PlannedShot`-ajat
verrataan takaisin aikasuunnitelmaan. Jos alennus ei säilytä vaadittua ajoitusta,
palautetaan diagnoosi; toista kamera-/leikkauskelloa ei luoda.

Ensiprofiili: 30–60 s, kaksi hahmoa, enintään kolme **tarinakohtausta** ja 16
lopullista otosta **reaktiot mukaan lukien**. Nykyinen `MAX_SHOTS = 64` koskee
kirjoitettuja otoksia, joista kukin voi tuottaa lisäreaktion. Adapterin 16 otoksen
raja on siksi tiukempi erillinen tarkistus. Jokainen tuotettu tekninen `Scene`
ei ole uusi tarinakohtaus. `expand_shots` liittää kohtaukset pohjan jatkoksi;
adapteri ei jätä sinne vahingossa toista kopiota samasta käsikirjoituksesta.

## Säännöt ja etusijajärjestys

Turvallisuus-/ajoitusrajojen rikkominen hylkää suunnitelman. Käyttäjän lukittu
rajaus ei ohita näitä rajoja. Kelvollinen lukitus ohittaa esteettisen valinnan.
Muuten sääntöjen etusija on taulukon pienimmästä numerosta suurimpaan; tasatilanne
ratkaistaan vakaan sääntö-ID:n ja otos-ID:n järjestyksellä. Kamerassa ei käytetä
satunnaisuutta. Syötteen `seed` välitetään lokiin ja muihin suunnittelijoihin,
mutta se ei muuta kovia ehtoja tai kameran tulosta.

| ID / etusija | Laukaisin ja päätös | Virhe tai hyväksyntä |
| --- | --- | --- |
| `CAM-001` / 0 | Epäkelpo aika, tuntematon kohde, puuttuva assetti tai vaadittu kamerapiirre jota nykyputki ei tue | Palauta lähdeviitteellinen virhe ennen renderöintiä; ei hiljaista rajausta tai laadun alentamista |
| `CAM-010` / 10 | Puherivi, tärkeä ele tai puhelimen näyttö | Älä katkaise repliikkiä tai peitä tarinan kannalta välttämätöntä sisältöä; puhe jatkuu kuvaleikkauksen yli vain erikseen tuetulla ja testatulla yhteisellä ääniraidalla |
| `CAM-020` / 20 | Tarinakohtauksen ensimmäinen kuva tai paikka vaihtuu | Näytä 2–4 s suuntaa antava laaja kuva, ellei käyttäjän lukittu ymmärrettävä aloitus täytä samaa tehtävää; aika sisältyy 30–60 s kokonaisuuteen |
| `CAM-030` / 30 | Kahden hahmon vastakuva | Pidä Aino ja Rane vakiintuneilla ruudun puolilla ja katseet keskustelukumppaniin; akselin ylitys vaatii näkyvän neutraalin uudelleenorientaation ja jäljen suunnitelmaan |
| `CAM-040` / 40 | Uusi puhuja tai vallanvaihto | Suosi medium-/close-up-kuvaa ajatuksen muutoksen kohdalla, ei joka virkkeen tai sanan kohdalla; katsojan reaktio saa oman, tarkoituksellisen tilan |
| `CAM-050` / 50 | Kuuntelijan paljastava reaktio | Varattu 0.6–1.5 s hiljainen reaktio; 1.5 s on yhteinen vähimmäisotos, paitsi käyttäjän erikseen lukitsema insertti. Reaktio kuluttaa kokonaisaikaa eikä venytä videota huomaamatta |
| `CAM-060` / 60 | Kasvot, suunta tai kädessä pidettävä esine | Varaa katseen suuntaan vähintään 10 % kuvaleveyttä; kasvojen ja tärkeän käden rajauslaatikot pysyvät 5 % reunamarginaalin sisällä. Tarkista liikeväli, ei vain alkukuvaa |
| `CAM-070` / 70 | Kilsat-insertti | Varaa yksi vakaa 5.0 s kuva. Näytä vain tarinan tarvitsema tieto oikeasta hyväksytystä UI-kuvasta; havainnekuva merkitään näkyvästi ja estää valmiin julkaisutason hyväksynnän |
| `CAM-080` / 80 | Tekstitys tai UI-teksti | Varaa alhaalta tekstitysalue; enintään kaksi tekstitysriviä, mitattu kirjasinasettelu, vähintään 4.5:1 kontrasti tekstin välittömään taustaan. Sama vaatimus testataan oikeassa vientiresoluutiossa |
| `CAM-090` / 90 | Kameraliike tukee ajatuksen muutosta | Ensiprofiilissa vain staattinen kuva tai 1.00–1.06 hidas push; korkeintaan yksi liikesuunta otosta kohti. Nykyiset preset-oletukset sopivat tähän. Ei käsivarakohinaa tai automaattista jatkuvaa zoomausta |
| `CAM-100` / 100 | Värimaailma ja valaistustunnelma | Valitse yksi lukittu väripaletti ja esivalmisteltu valaistu tausta; säilytä kasvojen erottuvuus ja valon suunta vastakuvissa. Jos paketti vaatii dynaamista valoa jota putki ei tue, ilmoita este |

Numeeriset sommittelu- ja luettavuuskynnykset ovat profiilin alustavia
hyväksyntäehtoja. Ne eivät ole väite elokuvastudioiden käyttämistä säännöistä tai
yksinään ammattitason kuvan mittareista. Kasvojen rajauslaatikon merkitys
(koko pää vai kasvojen näkyvä alue) määritellään asset-manifestissa yhdenmukaisesti.

Jos kuvan vaatimukset eivät mahdu yhtä aikaa, käsittele järjestyksessä:
poista koristeellinen push → valitse väljempi kelvollinen preset → erota
UI-insertti omaksi suunnitelluksi kuvakseen. Viimeinen vaihtoehto saa käyttää
vain jo varattua aikaa ja otosmäärää. Jos kokonaisuus ei mahdu, palauta
`CAM_LAYOUT_UNRESOLVED` tai `CAM_TIME_BUDGET`; puhetta ei nopeuteta tai tietoa
poisteta automaattisesti. Nykyinen shot-laajennin tukee vain kovia leikkauksia ja
hylkää yhtäaikaisen `composition.camera`-raidan: adapteri säilyttää tämän yhden
kameran omistajan mallin.

## Aino–Rane-esimerkki, 50 s

| Aika | Kuva ja tehtävä |
| --- | --- |
| 0–3 s | Laaja työpaikkakuva: Aino vasemmalla, Rane oikealla; kummankin katse suuntautuu keskusteluun |
| 3–12 s | Ranen medium, lievä push; käsky näyttää aluksi rutiinilta |
| 12–25 s | Ainon medium, kädet näkyvissä; hän pyytää täsmennystä ja tuo puhelimen esiin |
| 25–30 s | Vakaa Kilsat-insertti, yksi luettava merkintä; muu toiminta rauhoittuu |
| 30–43 s | Ranen vastakuva; selitys paljastaa ristiriidan, katse ja akseli säilyvät |
| 43–48.5 s | Ainon lähikuva, kuiva vastarepliikki |
| 48.5–50 s | Ranen reaktio; vitsi saa laskeutua ilman uutta puhetta |

Tämä on otosrakenteen ehdotus. Lopulliset leikkausajat johdetaan mitatuista
suomenkielisistä repliikeistä, eivät taulukon arvioluvuista. Insertti ei näytä
keksittyjä Kilsat-toimintoja eikä oleta puuttuvaa oikeaa UI-kuvaa saatavaksi.

## Hyväksyntä ja suorituskyky

Automaattiset testit: sama kanoninen syöte tuottaa identtisen otossuunnitelman ja
sääntöjäljen; reaktiot mukaan lukien otosmäärä on enintään 16; ajat kattavat
tavoitekeston ilman tahattomia aukkoja; puhe ja 5 s UI-varaus säilyvät
sample-tarkasti; tuntematon hahmo, NaN/ääretön arvo, kaksoiskamera,
ristiriitaiset lukitukset ja akselirikkomus tuottavat täsmälliset diagnoosit.
Sommittelutestit näytteistävät rajausten ja liikkeiden ääriarvot sekä tekstin
todelliset laatikot. Golden-kuvat ja käänteiset seekit tarkistavat yhteisen
esikatselu-/vientipolun; pelkkä suunnitelma-JSON ei riitä.

Taiteellinen hyväksyntä: ihminen katsoo oikean äänen ja kuvan pilotin sekä
pienennetyn jakeluversion. Tarkistetaan vitsin rytmi, katseet, eleiden näkyvyys,
kuvan jatkuvuus ja Kilsat-tekstin luettavuus. Lopputulosta ei hyväksytä
tuotantotasoiseksi vain siksi että numeeriset testit menivät läpi.

Viiden minuutin tavoite koskee myöhemmin nimettävää konetta ja valmiiksi
asennettua, lisenssiltään hyväksyttyä tyyli-/hahmo-/ääniympäristöä. Kameran
suunnittelulle varataan alustavasti enintään 2 s tästä yhteisestä budjetista;
tämä on suunnitteluraja, ei nykyinen mittaustulos. Kuvien valmistelu, ääni,
renderöinti, vienti ja tarkistus kuuluvat samaan 300 s kokonaismittaukseen.
Kameran suunnittelua ei ajeta käyttöliittymän tapahtumasilmukassa. Uuden
vaativan 3D-tyylin tai rigin luontia ei voi piilottaa esilämmityksen ulkopuolelle.
