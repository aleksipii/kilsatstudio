# Paikallinen päänseuranta

Paketoitu Mac-sovellus käynnistää apuohjelman vasta käyttäjän erillisestä
valinnasta. AVFoundation lukee kamerakuvaa ja Vision mittaa suurimman riittävän
varmasti tunnistetun kasvon asentoa. Mikrofoni, kuvan tai äänen tallentaminen,
verkkoyhteys ja ladattavat mallit eivät kuulu tähän toimintaan. macOS pyytää
kameran käyttöluvan tarvittaessa. Sovelluksen ja apuohjelman käyttöluvan
kuvaukset kertovat saman käyttötarkoituksen ja tietojen käsittelyn suomeksi.

## Komennot ja rajat

Ilman valitsimia apuohjelma palauttaa yhden JSON-vastauksen ja sulkeutuu.
Vastaus sisältää kallistuksen (`roll`) sekä mahdolliset kääntymisen ja
nyökkäyksen mittaukset (`yaw`, `pitch`) radiaaneina.

```sh
head_pose --stream --duration 60 --fps 12
```

Jatkuva seuranta kirjoittaa JSONL-rivejä, joissa `seq` alkaa nollasta,
`time` on kameran käynnistyksestä kulunut monotoninen aika sekunteina ja
`roll` sekä mahdolliset `yaw` ja `pitch` ovat mitattuja kulmia. Kesto on
1–60 sekuntia ja FPS-arvo 1–20. `--help` sulkeutuu onnistuneesti avaamatta
kameraa. Tuntematon, toistuva tai virheellinen valitsin palauttaa koodin 2
ennen kameran käyttöä. Valitsimien nimet ja JSON-avaimet säilyvät englanniksi.

Käyttöluvan ja kameran käynnistyksen aikaraja on 30 sekuntia. Jos kasvoja ei
voi mitata viiteen sekuntiin, seuranta päättyy. Jatkuva seuranta kestää enintään
valitun ajan. Apuohjelman viimeinen aikaraja on 92 sekuntia ja sovelluksen
95 sekuntia. Sovellus lukee rajatun määrän JSONL- ja virhetulostetta, tarkistaa
kulmat, järjestysnumerot ja ajat sekä lopettaa ja odottaa prosessin peruttaessa,
pysäytettäessä, virheellisen tiedon saapuessa tai aikarajan ylittyessä.
Apuohjelma sulkeutuu myös sen käynnistäneen sovelluksen sulkeutuessa.
Kamera ei avaudu sovelluksen käynnistyksessä.

Käyttöliittymä säilyttää yhden mittauksen tai enintään 30 sekunnin ja 600 näytteen
nauhoitusluonnoksen. Pysäyttäminen ei lisää liikettä animaatioon. Tarkistettu
luonnos lisätään erillisellä Käytä-toiminnolla mukautetun hahmon pään
avainruuduiksi yhdellä kumottavalla muokkauksella. Vain kaksiulotteinen
kallistus nauhoitetaan; `yaw` ja `pitch` ovat näkyviä mittaustietoja, eivät
vartalon tai kolmiulotteisen liikkeen kaappausta. Oletushahmojen päät tukevat
edelleen vain X/Y-siirtoa. Muokkauskontekstin vaihtaminen peruu kameran ja
hylkää käyttämättömän luonnoksen. Suora esikatselu muuttaa vain kutsujan
kloonaamaa Script-tietoa, ei tallennettua projektia tai hahmon juurta.

## Tarkistus fyysisellä Macilla

Seuraavat vaiheet vaativat oikean laitteen ja käyttäjän havainnot:

1. Avaa paketoitu sovellus Finderissä. Varmista, ettei kamera ole käytössä ennen
   mittauksen tai seurannan käynnistämistä. Avaa ja sulje Webkamera-osio
   seurannan aikana: käytön ilmaisimen sekä pysäytyksen ja peruutuksen pitää
   säilyä näkyvissä myös suljetun osion ulkopuolella.
2. Valitse mukautettu hahmo, jolla on pään nivel, ja pysäytä toisto. Käynnistä
   seuranta, hyväksy macOS:n käyttöluvan pyyntö ja tarkista kallistuksen suunta.
   Estä käyttö kerran ja varmista, että virhe kertoo, mistä lupa muutetaan.
   Kokeile uudelleen muutettuasi kameran käyttöluvan macOS:n asetuksissa.
3. Nauhoita lyhyt liike ja pysäytä nauhoitus. Projektin pitää säilyä ennallaan.
   Tarkista luonnos ja suunnan kääntö, sitten käytä liikettä. Yhden kumoamisen
   pitää poistaa kaikki nauhoitetut pään avainruudut säilyttäen hahmon juuren,
   pään X/Y-siirrot ja mittakaavan sekä muut aiemmin luodut avainruudut.
4. Vaihda lähdettä, kohtausta, hahmoa, toistopään paikkaa, välilehteä tai toistoa
   seurannan aikana. Varmista kameran sulkeutuminen ja vanhan esikatselun tai
   tuloksen hylkääminen. Peruutuksen pitää hylätä luonnos.
5. Poistu kuvasta yli sekunniksi nauhoituksen aikana ja varmista luonnoksen
   hylkääminen. Viiden sekunnin poissaolon pitää lopettaa seuranta. Kokeile
   toisen sovelluksen käyttämää kameraa ja ulkoisen kameran irrottamista.
   Prosessi ei saa jäädä pysyvästi käyntiin.
6. Kokeile kohtauksen jäljellä olevaa aikaa pidempää nauhoitusta. Virheen pitää
   estää lähteen osittainen muokkaus. Vie tarkistettu pään liike videoksi ja
   vertaa sitä esikatseluun.

CI kääntää, allekirjoittaa ja inventoi apuohjelman ja voi tarkistaa `--help`-
toiminnon sekä virheelliset valitsimet. Se ei käytä fyysistä kameraa eikä
vahvista käyttöluvan toimivuutta, liikkeen suuntaa tai kalibrointia,
kasvontunnistuksen luotettavuutta, taiteellista laatua tai käyttöliittymän
laitetoimintaa. Nämä ovat avoinna, kunnes käyttäjä kirjaa säilytettävät havainnot.

Applen ensisijaiset rajapintakuvaukset:

- https://developer.apple.com/documentation/vision/vnfaceobservation
- https://developer.apple.com/documentation/avfoundation/avcapturedevice/requestaccess(for:completionhandler:)
