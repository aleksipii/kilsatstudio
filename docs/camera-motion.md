# Jatkuva pään kallistuksen seuranta

Toteutus käyttää paketoidun Mac-sovelluksen paikallista AVFoundation/Vision-
aputyökalua. Seuranta käynnistyy vain käyttäjän painikkeesta ja macOS:n
kameraluvalla. Kuvia, ääntä tai verkkoliikennettä ei tuoteta. Yhden kallistuksen
kaappaus säilyy rinnalla.

## Työnkulku

Valitse Muokkaa-välilehdellä mukautettu hahmo, jolla on head-niminen nivel.
Pysäytä animaation toisto ja siirrä toistopää nauhoituksen alkukohtaan.
Käynnistä seuranta, odota tuoretta kasvomittausta ja aloita nauhoitus.
Pysäyttäminen tekee tarkistettavan luonnoksen. Vasta erillinen Lisää nauhoitus
-painike antaa sen pääsovelluksen yhteen kumottavaan lähdemuutokseen.

Tallennetaan ainoastaan 2D-kallistus. Yaw ja pitch näytetään mittaustietona;
niitä ei esitetä valmiina 3D- tai vartalomocapina. Kallistuksen suunta voidaan
kääntää ennen luonnoksen hyväksymistä. Alkuperäiset X/Y-, skaala- ja muut
nivelavainruudut säilyvät. Nauhoitus ei saa ylittää kohtauksen jäljellä olevaa
aikaa. Yli sekunnin mittauskatkos hylkää aktiivisen nauhoituksen.

## Rajat ja testaus

Stream-protokolla on JSONL, enintään 4096 tavua rivillä, 1200 mittausta ja
60 sekuntia. Mittaus käyttää todellista monotonista aikaa, ei saapumishetkestä
arvattua kuvaruutunopeutta. Nauhoitus on enintään 30 sekuntia ja 600 näytettä.
Apuohjelman asetusaikaraja on 30 sekuntia, kasvokatkon raja 5 sekuntia ja
emoprosessin kokonaisraja 95 sekuntia. Pysäytys/peruutus tappaa ja odottaa
apuprosessin. Näyttämön, valinnan, lähteen, ajan ja toiston muutokset sekä
pään ohjaimista poistuminen peruuttavat seurannan ja keskeneräisen luonnoksen.

`head_pose --help` ja virheelliset argumentit poistuvat ennen kameran avausta.
CI voi kääntää ohjelman ja tarkistaa nämä polut ilman kameran käyttöä.
Protokolla- ja avainruututestit eivät osoita fyysisen kameran, käyttöluvan,
kasvojen tunnistuksen, suuntakalibroinnin tai taiteellisen liikkeen hyväksyntää.
Ne tarvitsevat oikean Macin käyttäjän havainnot.
