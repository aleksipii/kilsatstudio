# Kilsat Studion käyttöohje

Ohje avautuu sovelluksen yläpalkin Ohjeet-painikkeesta. Päätoiminnot ovat
suomeksi; osa edistyneistä työkaluista ja virheilmoituksista on englanniksi.

## 1. Aloita tästä

Vasen Projekti-paneeli: luo jakso ja kohtauksia, valitse kohtaus ja hahmo.

Keskellä Näyttämö: näet valitun kohtauksen ja siirrät hahmoja hiirellä.

Alhaalla Aikajana: siirrä toistopäätä haluttuun aikaan ja toista animaatiota.

Oikealla Muokkaa: hahmon pää, avainruudut ja kuvituksen työkalut.
Puhe: vuorosanat, äänet ja Kokoron asennus. Vie video: valitun kohtauksen
vienti tai koko projektin kohtauskooste. Lisäasetukset: tekniset tarkistukset.

Ensimmäinen kokeilu: valitse hahmo näyttämöltä, siirrä aikajanan toistopäätä,
vedä hahmo uuteen paikkaan ja paina Toista liike. Tallenna projekti ennen sulkemista.

## 2. Avaa ja tallenna projekti

Avaa vasemman paneelin Luo animaatio vuorosanoista -osio tai MotionScript-välilehti.
Kirjoita projektitiedoston koko polku. Avaa-painike lukee juuri tämän polun;
se ei avaa tiedostonvalintaikkunaa.

Uuden projektin tallennus: anna Luo animaatio vuorosanoista -osiossa olemassa
olevan projektikansion koko polku sekä sen sisälle tulevan .kilsat-tiedoston
koko polku. Paina Tallenna projekti tai yläpalkin Tallenna.

Tallenna korvaa polussa olevan tiedoston. Käytä uutta tiedostonimeä, jos haluat
säilyttää vanhan version. .kilsat säilyttää myös projektin kuvituksen ja
resurssiviitteet; pelkkä YAML on käsikirjoitusta varten.

Kumoa peruu muokkauksen (Mac: Cmd+Z, muualla Ctrl+Z). Tee uudelleen palauttaa
sen (Cmd/Ctrl+Shift+Z). Kumoa ei poista levylle tuotuja resurssitiedostoja.

## 3. Uusi jakso ja kohtaus

Paina vasemmalla + Uusi jakso, anna nimi ja vahvista Luo jakso. Tallenna nykyinen
projekti ensin, jos haluat siitä erillisen tiedoston. Uusi jakso aloittaa uuden
projektin ja tyhjentää tallennuspolun. Kumoa palauttaa edellisen projektin
sisällön ja kuvituksen; anna tallennuspolku uudelleen tarvittaessa.

+ Uusi kohtaus lisää nykyiseen projektiin tyhjän 10 sekunnin kohtauksen nykyisillä
hahmoilla. Valitse haluttu kohtaus vasemmalta KOHTAUKSET-listasta.

Projektin lisätyökalut -osiosta löydät kohtauksen keston ja kuvakoon asetukset,
hahmojen lisäämisen sekä kohtausten kopioinnin, järjestyksen ja poiston.

## 4. Animoi hahmo hiirellä

Valitse näyttämön Hahmot-tila. Napsauta hahmoa tai sen nimeä vasemmalla.
Pysäytä toisto ennen muokkaamista.

Siirrä alhaalla aikajanan toistopää haluttuun aikaan. Vedä hahmo näyttämöllä
uuteen paikkaan ja vapauta hiiri näyttämön sisällä. Vapautus lisää sijainnin
avainruudun lähimpään kuvaruutuun. Siirrä toistopäätä ja vedä uudelleen:
sovellus laskee liikkeen avainruutujen välille.

Kumoa peruu koko vedon. Esc tai pudotus näyttämön ulkopuolelle peruu keskeneräisen
vedon. Myös kohtauksen, ajan, kameran tai näkymän koon vaihtuminen peruu vedon.

Tavallinen esikatselu liikuttaa hahmoa vedettäessä. Kuvitusta käyttävässä
koostetussa esikatselussa valintakehys liikkuu vedon aikana ja hahmon kuva päivittyy
vapautuksen jälkeen.

## 5. Pää ja webkamera

Valitse hahmo ja avaa oikealta Muokkaa. Pään asento -kohdassa X ja Y siirtävät
päätä. Paina Lisää pään avainruutu tallentaaksesi asennon toistopään kohdalle.

Oletushahmojen pää tukee X/Y-siirtoa. Kallistus ja kamerakaappaus edellyttävät
mukautettua nivelhahmoa, jolla on head-niminen nivel. Pelkkää kallistusta ei
näytetä oletushahmoille, koska niiden pää ei piirry kääntyvänä.

Mac-sovelluksessa: avaa Webkamera, paina Kaappaa pään kallistus ja salli kameran
käyttö macOS:n kysyessä. Asetu yksin kameran eteen. Sovellus mittaa yhden
kallistuksen paikallisesti ja sulkee kaappauksen. Kuvia tai ääntä ei tallenneta
eikä lähetetä verkkoon.

Tarkista tulos, käännä suunta tarvittaessa ja paina Kopioi kallistus luonnokseen.
Paina lopuksi Lisää pään avainruutu. Kaappaus ei muuta animaatiota automaattisesti.
Tämä ei ole jatkuva liikkeenseuranta. Fyysisen kameran ja rigien suuntakalibrointi
on vielä varmistettava oikealla Macilla.

Jos kamera ei avaudu, tarkista macOS:n Järjestelmäasetukset → Tietosuoja ja suojaus
→ Kamera. Kaappaus tarvitsee paketoidun Mac-sovelluksen aputyökaluineen.

## 6. Käsikirjoituksesta animaatioksi

Avaa vasemmalta Luo animaatio vuorosanoista. Kirjoita yksi vuorosana riville
muodossa HAHMON_TUNNUS: teksti. Käytä osiossa näkyviä projektin hahmotunnuksia.
Esimerkiksi EMMA: Hei! ja seuraavalle riville ALEX: Terve!

Luo kohtaus vuorosanoista tekee yhden kohtauksen. Sekuntia per vuorosana on
alustava kesto. Automaattiohjaa tekee sääntöihin perustuvan otosluonnoksen;
raja on kaksi puhujaa, 16 vuorosanaa ja 60 sekuntia. Nykyiset kohtaukset säilyvät.

Otosluonnoksen ajoitus on arvio, kunnes se tarkistetaan oikeasta puheesta.
Tarkista liikkeet, ilmeet, rajaukset ja repliikkien kestot itse. Toiminto ei
vielä takaa ammattitason taiteellista lopputulosta tai viiden minuutin tuotantoa.
Käsikirjoituksen YAML-avainsanat ovat englanniksi, vuorosanat voivat olla suomea
tai englantia.

## 7. Puhe ja suomenkielinen ääni

Avaa oikealta Puhe. Kokoron asennus -osiossa voit yhdistää olemassa olevan
paikallisen asennuksen tai asentaa englanninkielisen puheen mallin. Lataukset
käynnistyvät vain erillisellä suostumuksella. Asennus tarvitsee Python 3.10–3.12:n.

Valitse hahmon ääni ja paina Valmistele kohtauksen ääni. Odota valmistumista ja
paina alhaalta Toista ääni. Kokoron testipuhe on erillinen asennustesti;
se ei lisää vuorosanaa kohtaukseen.

Kokoro ei tue suomea. Suomenkielinen paikallinen Piper-polku on luvussa 11. Voit myös tuoda oman paikallisen
WAV-äänitteen valitun Speak-tapahtuman asetuksissa: valitse repliikki aikajanalta ja
avaa Muokkaa. Aseta äänitteen polku sekä tarvittava ajoitus. Valmistele ääni sen
jälkeen uudelleen. Tarkista suun ajoitus kuuntelemalla; tekstistä arvioitu ajoitus
ei ole mitattu kohdistus.

Yläpalkin Ääniesimerkki käyttää synteettisiä testiääniä, ei oikeaa puhetta.
Sen avulla voit kokeilla toistoa ja vientiä ilman mallin lataamista.

## 8. Kuvitus ja resurssit

Tallenna projekti ensin .kilsat-muodossa. Vasemman Projektin lisätyökalut → Resurssit -osion kautta tuot
paikallisen PNG/JPEG-kuvan tai muun tiedoston projektin kansioon. Anna resurssin
tunnus sekä tuotavan tiedoston koko polku.

Valitse näyttämöltä Kuvitus-tila ja oikealta Muokkaa. Kuvituksen työkalut löytyvät
sieltä. Resurssin tuominen luetteloon ei vielä sijoita kuvaa näyttämölle: lisää
kuvataso ja valitse sille tuotu kuvatunnus.

Resurssiviitteen poistaminen säilyttää alkuperäisen tiedoston. .kilsat-projektin
kansio ja siihen suhteutetut resurssit on pidettävä yhdessä siirrettäessä.

## 9. Vie video tai kohtauskooste

Valmistele ensin valitun kohtauksen ääni Puhe-välilehdellä. Myös äänetön liike
näkyy näyttämöllä ennen äänen valmistelua, mutta kohtauksen vienti tarvitsee
valmistellun kohtauksen.

Avaa oikealta Vie video. Valitse MP4 ja anna paikallisen FFmpeg-ohjelman koko
polku. Ohjelma ei lataa videokooderia automaattisesti. Anna uudelle tuloskansiolle
koko polku; kansiota ei saa olla vielä olemassa. Paina Vie kohtaus.

MP4 tarvitsee parillisen kuvaleveyden ja -korkeuden. Viennin mukana jäävät myös
PNG-kuvaruudut ja WAV-ääni. SVG/PNG-vaihtoehdot löytyvät samasta paikasta.

Automaattiohjauksen kaikki otokset tai useat kohtaukset viedään Koko projekti /
kohtauskooste -osiosta, ei vain valitun kohtauksen viennillä. Valmistele koko
kooste ennen vientiä. Osa koosteen asetuksista on vielä englanniksi.

Tekstitykset / Captions -valinta vaikuttaa esikatseluun ja vientiin muuttamatta
käsikirjoitusta. Seuraa käsikirjoitusta, näytä tai piilota tekstitykset.

## 10. Jos jokin ei toimi

Tarkista alapalkin tilaviesti. Näet pitkän viestin viemällä hiiren sen päälle.
Punainen virheilmoitus kertoo, mikä vaihe epäonnistui.

Muokkaus estyy toiston tai taustatyön aikana: keskeytä toisto tai odota työn
valmistumista. MotionScript-muokkauksen jälkeen paina Tarkista.

Puuttuva ääni: avaa Puhe, tarkista Kokoron asennus tai WAV-polku ja valmistele
kohtauksen ääni. Puuttuva MP4: tarkista FFmpeg-polku ja käytä uutta tuloskansiota.

Tekniset raportit, Metal-tarkistus ja puheen kohdistus löytyvät oikean paneelin
Lisäasetukset-välilehdeltä. Ne eivät ole tarpeen ensimmäisen hahmoliikkeen luontiin.

Mac-lataus: pura ladattu ZIP ja mahdollinen sen sisällä oleva ZIP. Avaa Kilsat
Studio Preview.app Finderissä. Lataaminen ei käynnistä sovellusta. Jos macOS
estää avaamisen, tarkista Tietosuoja ja suojaus -asetusten Avaa silti -toiminto.


## 11. Puheeseen ajoitettu ohjaus ja Full HD

Luo animaatio vuorosanoista -osiossa Ohjaa oikean puheen mukaan valmistaa
repliikkien oikeat WAV-äänet taustalla, mittaa näytemäärät ja lisää muokattavat
otokset, reaktiot ja kamerat. Voit antaa kullekin repliikille oman 24 kHz mono
PCM16 -WAVin. Tyhjä polku käyttää hahmon paikallista puheääntä. Anna ensin
olemassa oleva projektikansio; mitatut äänet tallentuvat sen assets-kansioon.
Kumoa palauttaa lähteen mutta ei poista tuotettuja WAV-tiedostoja.

Suomen puhetta varten valitse oikealta Puhe → hahmon puheääni → Piper, kieli
fi-fi ja asennetun profiilin tunnus, esimerkiksi piper_harri. Anna Piperin
asetustiedoston koko polku samalle välilehdelle. Uuden profiilin voit luoda siellä
valitsemalla asennetun Piper-ohjelman, ONNX-mallin, mallin JSON-asetukset ja
uuden profiilitiedoston polun. Välimuisti käyttää profiilin viereistä
piper-cache-kansiota. Tallenna profiili uutena JSON-tiedostona ja ota se käyttöön.
Profiilin vaihtaminen peruu keskeneräisen äänivalmistelun ja tyhjentää valmistellut
esikatselut. Valmistele sen jälkeen kohtauksen tai koko projektin ääni uudelleen.
Malli ja ohjelma asennetaan
erikseen; sovellus ei lataa niitä automaattisesti. Macin järjestelmä-äänten
haku on erillinen yksityisen esikatselun vaihtoehto, jota ei hyväksytä sarjan
julkaisuääneksi. Tarkista valitun mallin ja tuotannon käyttöoikeudet erikseen.
Automaattinen suun liike on myös suomeksi nimetty arvio; se ei ole mitattu
äänikohdistus.

Kun ohjaus on valmis, avaa Vie video → Koko projekti / kohtauskooste ja
valmistele kooste. MP4 Full HD · suoratoisto käyttää enintään 1920×1080 /
24 fps / 60 sekuntia ja kirjoittaa videota ilman PNG-välitiedostoja.
Kuvakokoa tai ruutunopeutta ei pienennetä automaattisesti. Viennin 300 sekunnin
määräaika koskee vientivaihetta; koko puheesta videoon -putken viiden minuutin
tavoite on mitattava erikseen.

## 12. Kameranauhoitus

Valitse mukautettu hahmo, jolla on head-nivel, ja avaa Muokkaa → Webkamera.
Käynnistä paikallinen seuranta erillisellä painikkeella. Kun tuore mittaus näkyy,
aloita nauhoitus, liikuta päätä ja pysäytä. Tarkista tulos ja lisää nauhoitus
erillisellä Käytä-painikkeella: pysäyttäminen ei vielä muuta animaatiota.
Nauhoitus lisää pään kallistuksen avainruudut yhtenä kumottavana muokkauksena.
X/Y-siirrot, mittakaava ja hahmon vartalon asento säilyvät.

Pidä nauhoitus kohtauksen jäljellä olevan keston sisällä. Toistopään, hahmon,
kohtauksen, lähteen tai välilehden vaihtaminen peruu keskeneräisen kaappauksen.
Kuvat ja ääni eivät tallennu; vain mitattu pään kallistus voidaan lisätä
animaatioon. Yaw/pitch ovat mittaustietoja, eivät 3D-vartalon liikkeenkaappausta.
Fyysisen kameran toiminta ja suuntakalibrointi on varmistettava oikealla Macilla.
