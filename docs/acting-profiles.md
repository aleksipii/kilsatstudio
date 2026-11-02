# Näyttelijäprofiilit, tunteen voimakkuus ja käsimuodot

Kolme lisäystä tekevät hahmoista toisistaan erottuvia ja tunteista vivahteikkaita.
Kaikki kolme ovat tavallista MotionScriptiä, muokattavissa ja deterministisiä:
esikatselu, aikahyppy ja vienti näyttävät saman ruudun.

Koodi: [`motion-script/src/acting.rs`](../crates/motion-script/src/acting.rs)
(profiilit ja voimakkuuden tarkistus),
[`motion-animation/src/acting.rs`](../crates/motion-animation/src/acting.rs)
(tunneasento, ryhti ja eleen koko),
[`motion-rig/src/hand_shapes.rs`](../crates/motion-rig/src/hand_shapes.rs)
(rigin käsimuodot) ja [`motion-scene/src/hands.rs`](../crates/motion-scene/src/hands.rs)
(käsimuodon valinta).

## Näyttelijäprofiili (`acting:`)

Hahmon oma tapa olla ja liikkua. Esiasetus tai lohko:

```yaml
characters:
  PASMA:
    acting: proud
  VEIKKO:
    acting: {preset: tired, energy: 0.9}
```

| Kenttä | Alue | Vaikutus |
| --- | --- | --- |
| `posture` | -1–1 | Ryhti: negatiivinen lysähtää (pää ja hartiat alas), positiivinen seisoo suorana. |
| `energy` | 0,5–1,5 | Eleiden koko (1 = sellaisenaan). |
| `expressiveness` | 0,5–1,5 | Kertoo jokaisen tunteen voimakkuuden. |
| `tempo` | 0,7–1,4 | Eleiden ja kävelyjen nopeus tarinaa käännettäessä. |

| Esiasetus | posture | energy | expressiveness | tempo |
| --- | --- | --- | --- | --- |
| `calm` | 0,2 | 0,85 | 0,85 | 0,9 |
| `lively` | 0,3 | 1,25 | 1,2 | 1,15 |
| `nervous` | -0,35 | 1,1 | 1,3 | 1,25 |
| `proud` | 0,8 | 1,05 | 0,9 | 0,95 |
| `tired` | -0,7 | 0,7 | 0,8 | 0,8 |
| `stoic` | 0,4 | 0,6 | 0,6 | 0,9 |

Ryhti, energia ja ilmeikkyys vaikuttavat kohtauksen arviointiin rigin kerroksena,
joten ne vaativat oman rigin (kuten kaikilla Tikku-ukkostudion hahmoilla).
Ilman rigiä profiili hylätään `Unsupported`-virheenä. `tempo` ei muuta arviointia:
tapahtumien ajat kirjoitetaan kerran, ja tarinakääntäjä lukee tempon ajoittaessaan
eleet ja kävelyt. Beatin `pace` kerrotaan hahmon tempolla.

Tikku-ukkostudion hahmojen profiilit: Noutaja `stoic`, Veikko `tired`,
Päsmä `proud`, Kipinä `lively`, Kiire `nervous` ja Sisu `calm`. Tarinan
`cast.<HAHMO>.acting` korvaa hahmon oman profiilin.

## Tunteen voimakkuus (`intensity`)

`emotion`- ja `speak`-tapahtumat sekä tarinan beatit ja reaktiot ottavat
valinnaisen voimakkuuden 0–1. Oletus 1 on täysi ilme kuten ennenkin.

```yaml
- {type: emotion, at: 2, actor: VEIKKO, value: angry, intensity: 0.3}
```

- **Asento** skaalautuu voimakkuuden mukaan. Iloinen nostaa rintaa ja päätä,
  surullinen laskee hartiat ja pään, vihainen työntää kyynärpäät ulos ja
  pään eteen, ja yllättynyt nostaa vartalon ja levittää kädet. Asento
  liukuu 0,25 sekunnissa kuten sisäänrakennetulla hahmolla.
- **Tehollinen voimakkuus** on voimakkuus kertaa hahmon ilmeikkyys, enintään 1,5.
- **Kasvot:** alle 0,3:n tehollisella voimakkuudella kasvot pysyvät neutraaleina
  ja tunne näkyy vain asennossa (alateksti). Siitä ylöspäin käytetään rigin
  piirrettyä ilmettä, ja myös kävelyn tunnelma seuraa samaa rajaa.
- `intensity` ilman tunnetta on virhe.
- Muutos aiempaan: kaikki omat rigit, joilla on vakionivelet (`torso`, `head`,
  `upper_arm_*`, `forearm_*`), saavat nyt tunneasennon jokaisesta
  tunnetapahtumasta. Aiemmin tunne vaihtoi rigeillä vain kasvot ja kävelyn.

## Käsimuodot

Rigi voi piirtää kädelle viisi muotoa: `fist`, `open`, `point`, `grip` ja
`thumbs_up`. Kädet määritellään käsiosien muunnelmina:

```yaml
hands:
  left: {part: hand_l, shapes: {fist: fist, open: open, point: point, grip: grip, thumbs_up: thumbs_up}}
  right: {part: hand_r, shapes: {fist: fist, open: open, point: point, grip: grip, thumbs_up: thumbs_up}}
```

Muoto valitaan automaattisesti, tärkein ensin:

1. Esinettä pitelevä käsi (`holds`, `hold`-tapahtuma tai P06-esine) **tarttuu** siihen.
2. Käynnissä oleva ele muotoilee käyttämänsä kädet, kun sen voimakkuus on yli
   0,35. Osoitus tekee osoittavan sormen. Vilkutus ja facepalm avaavat kämmenen.
   Peukutus nostaa peukalon, ja miettiessä käsi on nyrkissä. Kädet ristissä
   tekevät molemmista nyrkit. Olankohautus, taputus, kompastus ja hyppy avaavat
   molemmat kädet.
3. Vähintään 0,5:n voimakkuinen viha puristaa molemmat kädet nyrkkiin, ja yllätys avaa ne.
4. Muuten käsi on rento perusasennossa.

Jos rigi ei piirrä valittua muotoa, käytetään sen nyrkkiä ja muuten perusmuotoa.
Avoin käsi ei korvaudu nyrkillä. Tikku-ukkostudion kuudella hahmolla on kaikki
viisi muotoa piirrettyinä lapaskäsityyliin: sormet ovat yhtenä lohkona ja peukalo erikseen.

## Rajoitukset ja tarkistukset

- Automaattiset testit tarkistavat jäsennyksen, arvoalueet, kerrosten järjestyksen,
  käsimuotojen valinnan, kasvorajan, eleen koon, ryhdin, aikahypyn ja
  tarinan ajoituksen.
- Käsimuodot on katsottu SVG-ruuduista suurennettuina. Laajasta kuvasta ne
  erottuvat huonosti, koska tikkuhahmon käsi on pieni. Ne näkyvät parhaiten
  keski- ja lähikuvissa.
- Tunneasentojen ja profiilien taiteellinen arvio jaksoissa (luettavuus,
  komiikan ajoitus) puuttuu vielä, eikä sitä ole tehty fyysisellä Macilla.
- Sisäänrakennettu seitsennivelinen hahmo ilman rigiä ei käytä voimakkuutta,
  profiilia eikä käsimuotoja. Sen tunneasento toimii kuten ennenkin.
- Leikehahmoilla (`examples/cutout-cast`) ei vielä ole käsimuotoja. Ne saavat
  tunneasennon ja profiilin, kun niille annetaan `acting:`.
- Puhesynteesi ei käytä tunnetta eikä voimakkuutta äänen ilmaisuun.
