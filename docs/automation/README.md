# Käsikirjoituksesta ohjatuksi animaatioksi

Tila 2026-10-20: kymmenen agentin **suunnitelma** ja koneellisesti tarkistettava
pilottisopimus. Automaattiohjausta tai uutta animaatiorenderöijää ei tässä PR:ssä
toteuteta. Edistyminen suhteutetaan [master-progressiin](../master-progress.md).
Nykykoodin tarkastuksen pohja: `7bb5e21331a672b15be5b2227c93ccc8171e8fa7`.

Tavoitteena on alkuperäinen, visuaalisesti yhtenäinen, ilmeikäs tyylitelty
animaatio: selkeä näyttely, tarkoituksenmukaiset leikkaukset, luettava kuva,
uskottava ajoitus ja viimeistelty ääni. Pixar/Disney ovat käyttäjän antama
laadullinen vertailukohta. Vastaavaa tuotantotasoa tai viiden minuutin valmistumista
ei ole osoitettu tällä suunnitelmalla. Tuotteen M1–M6-virstanpylväät eivät sulkeudu
suunnitteludokumenttien tai tiedostotarkistusten perusteella.

## Kymmenen agentin vastuut

| Agentti | Suunnitelma | Päätulos |
| --- | --- | --- |
| 01 | [Käsikirjoitus](agents/01-script.md) | Lähdeviitteet, puhujat, intentit ja epäselvyydet |
| 02 | [Ohjaus](agents/02-direction.md) | Dramaturgia, jatkuvuus ja reaktiot |
| 03 | [Näyttely](agents/03-acting.md) | Katse, ilmeet, eleet, ennakointi ja asettuminen |
| 04 | [Aineistot](agents/04-assets.md) | Hahmopaketit, kyvykkyydet, versiot ja oikeudet |
| 05 | [Kamera](agents/05-camera.md) | Rajaukset, akseli, sommittelu ja appin luettavuus |
| 06 | [Puhe](agents/06-voice.md) | Todellinen ääni, ajoitus, välimuisti ja suunliikkeet |
| 07 | [Suorituskyky](agents/07-performance.md) | Työjono, määräaika, muistibudjetti ja Mac-mittaus |
| 08 | [Käyttöliittymä](agents/08-ux.md) | Tarkistus, valmius, tausta-ajo ja muokattava tulos |
| 09 | [Laatu](agents/09-quality.md) | Koneelliset portit ja erillinen taiteellinen arvio |
| 10 | [Integraatio](agents/10-integration.md) | Toteutusjärjestys, riippuvuudet ja sopimukset |

Agentit ovat suunnittelutyön jako. Valmis ohjelma ei tarvitse kymmentä verkossa
toimivaa tekoälyagenttia jokaiseen renderöintiin. Säännöt ovat paikallista,
versioitua ja testattavaa dataa; nykyinen offline-putki säilyy.

## Yhteinen sopimus

1. Rajattu suomenkielinen käsikirjoitus tai nykyinen MotionScript otetaan sisään.
   Vapaa proosa tuottaa tarkistettavan ehdotuksen. Epäselvä puhuja tai toiminto
   näytetään lähdekohdassa; sitä ei arvata onnistumisen nimissä.
2. `ScriptIntent v1` säilyttää alkuperäisen dialogin, UTF-8-tavuvälit ja lähteen
   SHA-256:n. Johdetulla päätöksellä on sääntö-ID.
3. `AssetManifest v1` varmistaa olemassa olevat, oikeuksiltaan tarkistetut paketit.
   `DirectionPlan v1` määrittää tavoitteen. Alustava otosjako ei lukitse
   repliikkiä lyhyemmäksi kuin sen todellinen ääni.
4. `AudioPlan v1` tuottaa tai tuo oikean äänen. Sen perusteella ratkaistaan
   lopulliset `ShotPlan v1`- ja `ActingPlan v1`-aikavälit.
5. Adapteri laskee tuloksen nykyisiksi `Script`-, `ShotList`-, `Event`- ja
   composition-rakenteiksi. Käytetään nykyisiä `expand_shots`- ja
   `Script::expand_rules`-toimintoja, validointia, yhteistä kohtausarviointia ja
   vientiä. Kilpailevaa renderöijää, MotionScript-jäsennintä tai kelloa ei luoda.
6. Laatuportit tarkistavat tuloksen. Käyttäjä voi muokata lähdettä ja otoksia
   sekä käyttää varmennettuja välimuistiosumia seuraavassa ajossa.

Yhteiset rajat: 30–60 s, enintään kaksi hahmoa, kolme **tarinakohtausta**,
64 beatia ja 16 **lopullista otosta reaktioineen**. Nykyinen otoslaajennus tekee
teknisen `Scene`-rakenteen jokaisesta otoksesta; sitä ei lasketa kolmen
tarinakohtauksen rajaan. Nämä ovat ehdotetun nopean profiilin rajoja.

Intentissä ajat ovat kohtauspaikallisia sekunteja. Ratkaistun suunnitelman
kanoninen aika on kokonaislukuisia 24 kHz:n näytepaikkoja. Kohtauspaikalliset ja
koko elokuvan ajat muunnetaan kerran eksplisiittisillä kohtausoffseteilla.
Väli on `[start,end)`. Eleiden ruutukvantit johdetaan näytekellosta; puhetta
ei lyhennetä ruutupyöristyksen tai arvioidun puhenopeuden perusteella.

Ratkaisujärjestys: käyttäjän lukitus → eksplisiittinen validoitu tieto → valitun
paketin sallittu oletus → yleinen sallittu oletus. Ristiriita samalla tasolla
antaa virheen. Kyvykkyys-, oikeus- ja resurssirajat tarkistetaan aina.
Seed valitsee vain samanarvoisia visuaalisia variantteja. Lopulliset vaikutukset
järjestetään nykyisten sääntöjen nimijärjestyksellä; uudet prioriteetit eivät muuta
olemassa olevan `expand_rules`-funktion semantiikkaa.

## Viiden minuutin tavoite

| Vaihe | Ehdotettu yläbudjetti |
| --- | ---: |
| Syöte ja validointi | 5 s |
| Ohjaus, kamera ja ratkaisun suunnittelu | 10 s |
| Aineistojen varmennus | 10 s |
| Todellinen puhe | 60 s |
| Suunliikkeiden ajoitus | 20 s |
| Näyttelyn muodostus | 20 s |
| Renderöinti | 145 s |
| Koodaus ja lopputarkistukset | 20 s |
| Vara | 10 s |
| Yhteensä | **300 s** |

Nämä ovat budjetteja, eivät mittaustuloksia. Agenttien 2–5 sekunnin alatehtävät
sisältyvät taulukon vaiheisiin. Sarjallinen summa on 300 s. Rinnakkaisuutta
hyödynnetään todellisten riippuvuuksien ja muistirajojen puitteissa.
60 s / 24 fps tarkoittaa 1440 ruutua; renderöinti tarvitsee vähintään noin
9,94 ruutua/s 145 sekunnin vaiheessa.

Mittaus alkaa käyttäjän käynnistyksestä ja päättyy varmennettuun videotiedostoon.
Jonotus, välimuistin tarkistus, ääni ja koodaus sisältyvät tulokseen. Puuttuvat
aineistot tai tarkistusta vaativa syöte estävät nopean profiilin käynnistyksen.
Ajon keskeytyessä käyttäjän tarkistukseen tai 300 s ylittyessä SLA ei täyty.
Kylmäkäynnistys ja lämmin ajo raportoidaan erikseen. Laatua, resoluutiota tai
puhetta ei vaihdeta salaa nopeuden saavuttamiseksi.

Tavoiteprofiili on 1920×1080 / 24 fps ja todellinen puhe. **Se on nykyisellä
viennillä estynyt:** 30–60 s ylittää `motion-export`-moduulin miljardin
rasteripikselin kokonaisrajan. Muistirajat säilyttävä suoratoistovienti pitää
toteuttaa ja mitata ensin. Erillinen 960×540 / 24 fps -referenssiprofiili mahtuu
pikselirajaan 60 sekunnilla. Se ei osoita Full HD -laatua tai viiden minuutin
valmistumista. Katso [vientirajat ja mittaus](agents/07-performance.md).

## Pilotti ja CI

[Uusi suomenkielinen 50 sekunnin pilotti](../../examples/automation/pilot-fi.txt)
on kirjoitettu tätä suunnitelmaa varten. Se ei ole kadonneen sarjapaketin
palautettu kopio. Aino ja Rane väittelevät siitä, kenen kustannuksia oma auto
säästää. Kilsat näkyy viiden sekunnin insertissä.

[Esimerkki-intentti](../../examples/automation/pilot-intent.json) on käsin tehty,
lähteeseen sidottu suunnittelufixtuuri. Studio ei vielä lue tätä formaattia.
Hahmoaineistot, oikea Kilsat-kaappaus ja suomalaiset äänitteet puuttuvat.
Nykyinen `SpeechRequest` tukee `en-us`/`en-gb`; suomea ei ohjata niihin
korvikkeena. Tarvitaan hyväksytyt tuodut äänet tai erikseen toteutettu ja
varmennettu suomenkielinen puhepalvelu.

```sh
python3 tools/check_animation_plan.py --report out/animation-plan-report.json
python3 -m unittest discover -s tools/tests -p 'test_animation_plan.py' -v
```

CI tarkistaa kymmenen raportin olemassaolon, 300 sekunnin budjettisumman,
lähteen hashin ja tavuvälit, puhujat, rajat, puheen ajoitusristiriidat, otospeiton
ja rehelliset tilat. Negatiiviset testit tarkistavat virheiden hylkäykset.
`design_fixture_valid` ei tarkoita, että proosa tulkittiin, ääni syntetisoitiin,
video renderöitiin tai nopeus/laatu hyväksyttiin.

## Tehty ja toteutettavat työt

- [x] Kymmenen suunnitteluraporttia ja nykykoodin rajojen tarkastus.
- [x] Yhteiset sopimukset, sääntöjen etusijat ja vaihebudjetit.
- [x] Suomenkielinen pilotti sekä lähde-/aikafixtuuri.
- [x] Suunnittelusopimuksen tarkistin, negatiiviset testit ja CI-määritys.
- [ ] Rust-tuontiadapteri, täydellinen versioitu skeema ja lähdeviitteiden tallennus.
- [ ] Ohjaus-, näyttely- ja kameraratkaisin nykyisiin aikajanoihin.
- [ ] Aino/Rane-hahmopaketit, propit, miljöö ja oikea Kilsat-kaappaus.
- [ ] Suomenkielinen puhepolku, mitattu suunliikelaatu ja äänen viimeistely.
- [ ] Suoratoistava Full HD -vienti sekä määräaika ja peruutus koko ajolle.
- [ ] Käsikirjoituksen tarkistusnäkymä ja muokattavat automaattisuunnitelmat.
- [ ] Fyysisen Macin nopeus, taiteellinen pilottiarvio ja julkaisun oikeudet.

Toteutusten riippuvuudet ja hyväksyntäehdot ovat
[integraatiosuunnitelmassa](agents/10-integration.md). Tuotantoon pääsy tarvitsee
erikseen [laatuarvion](agents/09-quality.md). Suunnitelman tai CI-fixtuurin
valmistuminen ei sulje master-progressin muita avoimia töitä.

