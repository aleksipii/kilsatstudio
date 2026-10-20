# Agentti 02 — ohjaus, näyttämöllepano ja leikkaus

Tila: **suunnitelma**, ei toteutettu automaattiohjaaja eikä hyväksytty laatutulos.
Omistettu alue: `ScriptIntent v1 → DirectionPlan v1 → nykyinen MotionScript`.
Viiden minuutin tavoite koskee vain ennalta valmisteltua 30–60 sekunnin
kaksihahmoista tyyliteltyä tuotantoprofiilia. Säännöt tavoittelevat selkeää
elokuvallista kerrontaa; ne eivät osoita Pixar- tai Disney-tuotannon laatua.

## Nykyinen pohja ja puuttuva osa

Tarkastettu lähde: main `7bb5e21331a672b15be5b2227c93ccc8171e8fa7`.

| Nykyinen rakenne | Todellinen vastuu | Tämän suunnitelman lisäys |
| --- | --- | --- |
| `crates/motion-script/src/shots.rs`: `ShotPreset`, `ShotSpec`, `CutRules`, `ShotList::from_yaml` | Tiukka otoslista; establishing/medium/close_up/over_the_shoulder/reaction_hold, käsin annettu puheen kesto | Merkityksellisten beatien muuttaminen rajatuksi otoslistaksi |
| `shots::expand_shots`, `ShotExpansion`, `PlannedShot` | Puhdas deterministinen laajennus tavallisiksi sceneiksi ja absoluuttiseksi leikkaussuunnitelmaksi; kameran, sijoittelun ja puheen validointi | Kerronnallinen tavoite, lähdeviitteet ja jatkuvuuden tarkistus ennen laajennusta |
| `shots.rs`: `Stage::position`, `camera_keys`, `stage_tracks` | Kohdehahmon paikkaa seuraava rajaus; `place` ja `off_stage`; kameran avaimet liikkeen rajoilla | Sommittelun turva-alueet ja akselin säilyttämisen suunnitelma |
| `crates/motion-script/src/rules.rs`: `Script::expand_rules` | Ehdollinen clip/kamera-laajennus vain alkuperäisistä tapahtumista, nimijärjestyksessä; ei rekursiota | Valmiin ohjaussuunnitelman muuntaminen olemassa oleviin tapahtumiin; ei uutta ajonaikaista sääntökieltä |
| `lib.rs`: `Composition`, `Script::validate` | Muun muassa näyttelijä-, nivel-, luu- ja katseraidat sekä nykyinen semanttinen validointi | Ohjaajan omien kerronta- ja jatkuvuusrajojen tarkistus nykyisen validoinnin lisäksi |

Nykyinen otoskirjasto ei lue äänitiedostoja eikä syntetisoi puhetta. Annettu
`ShotLine.duration` voi olla luonnoksen arvio; viimeistellyn suunnitelman kesto
on korvattava oikean audion kestolla. Automaattista käsikirjoituksen tulkintaa,
kerronnallista otosvalintaa tai kuvien visuaalista laadunhyväksyntää ei tällä
lähdeluvulla ole osoitettu.

## Rajapinta ja rajat

Syöte on validoitu `ScriptIntent v1`: enintään kaksi pysyvällä tunnisteella
nimettyä hahmoa, 1–3 **tarinakohtausta**, 30–60 sekunnin tavoite, järjestetyt
beatit, puhujat, toiminnat, mahdolliset ohjaajan lukitukset ja lähdetekstin
viitteet. Ohjaus ei päättele ironiaa tai tekijän tarkoitusta pelkästä sanasta;
epäselvä beat saa turvallisen neutraalin ehdotuksen ja näkyvän huomautuksen.

Ehdotettu `DirectionPlan v1` sisältää `version`, `source_hash`, `ruleset_hash`,
`asset_manifest_hash`, `seed`, `duration_samples`, `sample_rate`, `shots`,
`continuity` ja `diagnostics`. Jokaisessa otoksessa on pysyvä `shot_id`,
`story_scene_id`, `beat_id`, `source_span`, `rule_ids`, globaali alku/loppunäyte,
puhujan tunniste, rajaus, kamerakohde, sijoittelu, lukitukset ja perustelu.
Suunnitelma on tallennettava ja muokattava ennen renderöintiä. Näytteet ovat
yhteisen äänikellon kokonaislukuja; MotionScript-sekunnit muodostetaan samasta
suhteesta muunnosrajalla.

Tämän nopean profiilin ehdotetut tuotantorajat ovat enintään 16 lopullista
otosta, mukaan lukien reaktio-otokset, sekä enintään kolme tarinakohtausta.
Nykyinen `expand_shots` tuottaa **yhden teknisen Scenen per otos**, joten nämä
rajat eivät tarkoita korkeintaan kolmea laajennetun Scriptin sceneä. Nykyinen
kirjasto sallii 64 pääotosta ja lisäreaktioita; automaattiprofiili käyttää
tiukempaa yhteistä 16 otoksen laskuria ennen laajennusta.

## Säännöt ja etusija

Korkein etusija on 1. Samanaikaiset ehdokkaat ratkaistaan järjestyksellä
`(etusija, beatin lähdejärjestys, rule_id, actor_id)`. Lähdekäsikirjoituksen
lukitusta ei ohiteta alemman tason sommittelusäännöllä. Kahden lukituksen
ristiriita estää ajon. Variantteihin ei käytetä ympäristön satunnaisuutta:
ehdotettu valinta on `SHA256(ruleset_hash || seed || beat_id || rule_id)`
kiinteästä aakkosjärjestyksessä olevasta hyväksyttyjen varianttien luettelosta.
Nykyinen shot-laajennus ei käytä satunnaisuutta lainkaan.

| ID / etusija | Ehto → toiminta | Epäonnistuminen ja esimerkki |
| --- | --- | --- |
| DIR-01 / 1 | Säilytä puhe ja käyttäjän lukitut ajat; varaa oikean PCM-audion näytemäärä kokonaisena | Jos puhe ja pakolliset toiminnot eivät mahdu 60 sekuntiin: `DIALOGUE_OVERFLOW`; ei puheen katkaisua tai automaattista nopeutusta |
| DIR-02 / 1 | Hahmotunniste, kohde ja tunnettu asset ovat olemassa; puhuja pysyy kuvauskelpoisessa tilassa | `UNKNOWN_ACTOR` tai `MISSING_ASSET` estää lopullisen tuotannon; ei kolmatta hahmoa, kertojaa tai keksittyä puhetta |
| DIR-03 / 2 | Säilytä käyttäjän shot-, sijoittelu- ja kamera-lukitukset, jos ne läpäisevät rajat | Lukittu kameraraita ja automaattinen shot-kamera eivät saa kilpailla; `CAMERA_CONFLICT` lähdeviitteellä |
| DIR-04 / 3 | Ensimmäinen beat uudessa paikassa → 1,5–3 sekunnin yhteiskuva, tarvittaessa puheen aikana | Ei lisätä aikaa jo täyteen aikatauluun; jos rajaus ei näytä molempia, käytä hyväksyttyä leveää profiilia tai `FRAMING_UNRESOLVED` |
| DIR-05 / 3 | Aseta Aino vasemmalle ja Rane oikealle ensimmäisessä yhteiskuvassa; säilytä järjestys saman tarinakohtauksen ajan | Akselin ylitystä ei arvata; käsikirjoitettu vaihto tarvitsee näkyvän leveän siirtymäotoksen tai käyttäjän lukituksen |
| DIR-06 / 4 | Puhujan tavoite tai valtasuhde muuttuu → medium; tärkeä paljastus tai loppurepliikki → close_up | Sama puhuja ja sama tavoite jatkavat samaa kuvaa. Enintään yksi tarkoituksellinen hidas push samassa otoksessa |
| DIR-07 / 4 | Vastapuolen reaktio on eksplisiittinen beat → reaction_hold 0,4–1,2 sekuntia, jos aikabudjetti sallii | Ei lisätä reaktiota jokaisen repliikin jälkeen. Jos ei mahdu, säilytä kuuntelija samassa kuvassa ja raportoi valinta |
| DIR-08 / 4 | Kilsat-todiste on tarinan käänne → eksplisiittinen puhelimen/ruudun insert-beat, profiilin mukaan 4–6 sekuntia | Oikean käyttöliittymäaineiston puuttuessa vain merkitty havainne luonnokseen; julkaisu estyy asetuksella `require_real_app_ui` |
| DIR-09 / 5 | Repliikin ja toiminnan turva-alueet, katseen tila ja pään ylätila mahtuvat hyväksyttyyn kuvasuhdeprofiiliin | Kiinteä fallback: medium → establishing; jos kumpikaan ei kelpaa, `SAFE_FRAME_FAILED`, ei rajauksen hiljaista hyväksyntää |
| DIR-10 / 5 | Toiminta tai jälkireaktio motivoi leikkauksen; vähimmäisotos 1,5 sekuntia, paitsi erikseen lukittu insert | Alle rajan olevat peräkkäiset saman tavoitteen kuvat yhdistetään. Jos 16 kuvaa ylittyy vielä, `SHOT_BUDGET_EXCEEDED` |
| DIR-11 / 6 | Jäljelle jäävä hiljainen aika käytetään rajattuihin loppu- tai reaktiopitoihin; sarjan rytmi on kuiva | Ensimmäisenä poistetaan vain vapaaehtoiset tauot, ei lähdetekstin määräämää toimintaa. Alle 30 sekunnin sisältöä ei venytetä loputtomalla seisomisella |

Aino on samaistuttava ja pätevä; Rane käyttää pikkumaista valtaa. Tämä näkyy
esimerkiksi Raneen kohdistuvana omahyväisenä medium-kuvana ja Ainon rauhallisena
vastakuvana. Sääntö ei saa muuttaa Ainoa jatkuvasti avuttomaksi eikä käyttää
automaattista lähikuvaa jokaisen kirosanan kohdalla. Näyttelyn tarkat eleet
kuuluvat näyttelysuunnitelmalle, jonka kameraa ohjaavat vain hyväksytyt
toimintamerkit.

## Puheen aikataulu ja nykyiseen polkuun laskenta

1. Lukitse alkuperäinen repliikkijärjestys ja haetun/tuotetun audion tarkat
   näytemäärät. Arvioitu puheaika merkitään `estimated`, eikä sitä hyväksytä
   lopullisen puheen tarkastukseksi.
2. Aseta pakolliset toimintajaksot ja lähteen lukitut tauot. Laske puheen,
   näiden toimintojen ja pakollisten inserttien yhdisteen kesto, älä pelkkää
   summaa: insert voi tapahtua puheen aikana, jos lähde ja kuvan luettavuus
   sen sallivat. Päällekkäinen eri puhujien puhe vaatii eksplisiittisen tuen;
   nopean profiilin oletus on vuoropuhelu ilman päällekkäisyyksiä.
3. Valitse otokset yllä olevilla säännöillä ja sovita vain valinnaiset pidot.
   Ei uusia repliikkejä, tapahtumia tai semanttisia lyhennyksiä sovittamisen takia.
4. Muunna hyväksytyt otokset nykyisiksi `ShotSpec`-arvoiksi ja käytä
   `expand_shots`. `ShotLine.duration` = oikea PCM-kesto. `lead_in` ja `hold`
   johdetaan valmiista suunnitelmasta; oletusarvoinen tail 0,5 sekuntia ei saa
   vahingossa pidentää jokaista jo ajoitettua repliikkiä uudelleen.
5. Nykyinen otosformaatti lisää yhden päärepliikin per otos. Usean repliikin
   yhteiskuva tarvitsee joko nykyisten `events`-puhetapahtumien eksplisiittisen
   käsittelyn ja erillisen keston tarkistuksen tai vähimmäistoteutuksessa omat
   peräkkäiset samoin rajatut otokset. Tätä ei saa väittää valmiiksi pelkällä
   `ShotSpec.line`-kentällä.
6. Tarkista `ShotExpansion.plan` verrattuna näytekelloiseen suunnitelmaan ja
   `Script::validate`. Nykyisen `expand_rules`-kameratehosteita ei kohdisteta
   samoille ajoille shot-kameran kanssa: generoitu shot-kamera on ainoa omistaja.
   Kameraristiriita palautetaan virheenä. Nykyiset clip-säännöt saavat edelleen
   laajeta vain sallittuihin rajattuihin tapahtumiin.

Nykyinen shot-kirjasto tukee vain kovia leikkauksia. Uusi ohjaaja käyttää niitä
ensimmäisessä profiilissa; häivytyksiä tai ristileikkauksia ei luvata tämän
adaptoinnin tuottamiksi.

## Aino–Rane-pilotin ohjausesimerkki

Seuraava on uusi lyhyt **suunnitteluesimerkki**, ei kadonneen 31 jakson paketin
palautus. Käsikirjoituksen käänne: Rane kutsuu työajoa Ainon omaksi ajaksi;
Aino näyttää ajopäiväkirjan merkinnän ja Rane joutuu perustelemaan puheensa.

| Beat | Tarkoitus | Sääntöjen ehdotus |
| --- | --- | --- |
| Pyyntö | Rane: ”Tämä on ihan pieni keikka. Hoida omalla ajalla.” | Yhteiskuva paikan ja valtasuhteen avaamiseksi; DIR-04/05 |
| Tarkennus | Aino: ”Työkalut, työauto ja sinun asiakkaasi?” | Ainon medium; puhe kokonaisena; DIR-01/06 |
| Selitys | Rane: ”Ajattele sitä joustavuutena.” | Rane medium, harkittu pieni push vain jos kuvabudjetti sallii |
| Todiste | Aino näyttää Kilsat-merkinnän: ”Minä merkitsin sen työajoksi.” | 4–6 sekunnin luettava insert; oikea UI vaaditaan julkaisuun; DIR-08 |
| Seuraus | Rane: ”Älä nyt tee tästä numeroa.” Aino: ”En. Ajopäiväkirja teki.” | Vastakuvat ja lyhyt Rane-reaktio; ei uutta narratoria tai toistettua punchlinea |

Taulukko ei aseta virheellisesti 50 sekunnin kellotusta muutamalle repliikille.
Tarkat kestot ja mahdollinen lisätoiminta ratkaistaan vasta oikean audion ja
täydellisen pilotin käsikirjoituksen perusteella. Liian lyhyt aineisto palauttaa
`SCRIPT_UNDERFILLED`-huomautuksen eikä saa ammattitason hyväksyntää.

## Hyväksyntätestit ja työbudjetti

Ohjausvaiheen ehdotettu katto on 5 sekuntia valmiiksi validoidulle syötteelle;
se on viiden minuutin kokonaisbudjetin osa, ei tämän dokumentin mittaustulos.
Säännöt muodostavat yhden rajatun läpikäynnin ja korkeintaan yhden sommittelun
fallbackin per otos. Ei avointa optimointisilmukkaa, verkkohakua tai malliajoa.

- Sama kanoninen syöte, säännöstö, asset-versiot ja seed tuottavat bittitasolla
  saman DirectionPlanin, myös eri hajautustaulujen lisäysjärjestyksillä.
- Rane–Aino–Rane-keskustelussa akseli ja tunnisteet säilyvät; paikan vaihto
  nollaa vain kyseisen tarinakohtauksen jatkuvuustilan.
- 59,8 sekunnin pakollinen puhe ei saa 0,5 sekunnin oletustailia joka riville;
  60 sekunnin ylitys todetaan ennen renderiä, puhetta ei leikata.
- Oikean PCM-keston muuttuminen invalidioi riippuvat leikkaukset. Arvioidun
  keston välimuistiosuma ei voi pitää vanhaa leikkuusuunnitelmaa voimassa.
- `ShotExpansion.plan` kattaa aikajanan ilman aukkoja, negatiivisia kestoja tai
  tuplattua lead-inia; aikavirhe on enintään yhden yhteisen ääninäytteen luokkaa
  muunnosrajan pyöristyksessä, ei kasvava sekuntivirhe repliikkiä kohti.
- Lukittu kamera vastaan automaattinen kamera, puuttuva hahmo, tyhjä puhe,
  NaN-aika, yli 16 lopullista otosta ja 4. tarinakohtaus hylätään lähdeviittein.
- Vain elokuvallisen numerotarkastuksen läpäisy ei riitä: pilotin sokkoarvio
  tarkistaa luettavan vitsin, jatkuvuuden, tahattoman leikkauksen, katseen ja
  rajauksen oikeista kuvista. Ihmishyväksyntä sekä renderin ja äänen laatu
  säilyvät erillisinä portteina.

Tässä agenttityössä luettiin nykyistä lähdekoodia ja kirjoitettiin suunnitelma.
Uutta ohjausmoottoria, viiden minuutin renderiä tai fyysisen Macin hyväksyntää
ei ajettu.
