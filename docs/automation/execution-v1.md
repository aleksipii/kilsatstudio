# Automaattiohjauksen ensimmäinen toteutus

Tämä on rajattu paikallinen ohjausluonnos, ei ammattitason laadun hyväksyntä.
Orkestroija käyttää olemassa olevaa MotionScript-validointia ja otoslaajennusta;
se ei käynnistä kielimallia, puhesynteesiä tai uutta renderöijää.

## Työnjako

| Vastuu | Omistettu toteutus | Luovutus |
| --- | --- | --- |
| Orkestroija | `auto_direction/orchestrator.rs` | Syöte → tulkinta → ajoitus → otokset → näyttely → validoitu MotionScript |
| Käsikirjoituksen tulkinta | `auto_direction/parser.rs` | Alkuperäiset vuorosanat, hahmot, eksplisiittiset tunnetilat ja lähderivit |
| Puheen ajoitus | `auto_direction/timing.rs` | Rajatut 24 kHz:n kokonaislukuiset näyteajat ja ajoituksen alkuperä |
| Ohjaus ja kamera | `auto_direction/director.rs` | Nykyinen `ShotList`, vakaat sijoittelut ja kuvarajaukset |
| Näyttely | `auto_direction/acting.rs` | Nykyiset ilme-, ele- ja mahdolliset katserakenteet |
| Laadunvarmistus | Integraatio- ja yksikkötestit | Determinismi, rajat, lähteen säilyminen ja muokattavuus |
| Integraatio | Pääagentti | Julkinen moduuli, työpöytäkäyttöliittymä, tarkistukset ja GitHub-toimitus |

Tiedostojen omistus erottaa rinnakkaiset muutokset. Yhteiset tyypit ovat
`auto_direction/mod.rs`:ssä; orkestroija ei kirjoita niitä muiden yli.

## Orkestroijan sopimus

`direct(source, dialogue, supplied_samples)` palauttaa uuden lähteen,
ensimmäisen lisätyn kohtauksen indeksin, otosmäärän, lisätyn kokonaiskeston
näytteinä, ajoituksen alkuperän sekä näkyvät huomautukset. Toiminto ei kirjoita
levylle eikä korvaa nykyisiä kohtauksia. Nykyinen `expand_shots` lisää yhden
teknisen kohtauksen jokaista otosta kohti; käyttöliittymä vastaa kumoamisesta ja
projektin tallennuksesta. YAML:n muotoilu ja kommentit normalisoituvat.

Yhteiset rajat ovat 16 vuorosanaa/otosta, kaksi puhuvaa hahmoa, 16 KB:n
dialogisyöte, 100 KB:n työpöytälähde ja enintään 60 sekunnin lisätty kokonaisuus.
Alle 30 sekunnin luonnos saa huomautuksen; sen kestoa ei täytetä hiljaisuudella.
Uudet reaktio-otokset edellyttävät erillistä yhteisen otosbudjetin laajennusta.

Ilman annettuja kestoja ajoitus on aina tekstistä arvioitu. Annetut kestot ovat
kutsujan väittämiä 24 kHz:n näytemääriä, eivät tämän moduulin tarkistamaa PCM:ää.
Otoskääntäjän tuottamia puheen alkuja ja kestoja verrataan ajoitukseen yhteisellä
näytekellolla. Virhe pysäyttää koko luonnoksen; puhetta ei leikata sopivaksi.

## Jäljelle jäävä työ ja hyväksyntä

- Oikean äänen valmistelun ja automaattisen uudelleenajoituksen yhdistäminen
  ohjausluonnokseen; suomenkielinen Kokoro-synteesi ei sisälly tähän.
- Alkuperäisen kymmenen agentin suunnitelman täysi intentti-, lukitus-, aineisto-
  ja laatumalli. Tämä moduuli ei väitä toteuttavansa niitä kokonaisuudessaan.
- Fyysisen Macin käyttöliittymä-, ääni-, kamerarajaus- ja suorituskykyhavainnot.
  Viiden minuutin tavoite on mitattava oikealla tuotantoputkella.
- Taiteellinen katselmointi oikeista renderöidyistä kuvista ja todellisesta
  puheesta: selkeä toiminta, jatkuvuus, ajoitus, näytteleminen ja vitsin luettavuus.
- Tuotantolaatu ja Pixar/Disney-vertailutaso eivät seuraa rakennetestien
  läpäisystä. Todelliset testitulokset ja jakelupaketti kirjataan PR:ään erikseen.
