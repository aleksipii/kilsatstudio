# Paperinukkehahmot

Kolme omaperäistä hahmoa paperileikkotyyliin: iso pyöreä pää suoraan pienen
vartalon päällä, töpöt lapaskädet ja -jalat, tasaiset värit ja paksu tumma
ääriviiva jokaisen palan ympärillä. Hihat ja lahkeet taipuvat yhtenä palana,
muut osat ovat jäykkiä. Suu ja silmät vaihtuvat valmiista piirroksista, ja pää
kääntyy vaihtamalla piirrosta. Mukana on lyhyt, **äänetön** katukohtaus. Mitään ei
ladata verkosta.

## Avaa sovelluksessa

1. Avaa `cutout-cast.kilsat` projektin avauskentästä.
2. Valitse kohtaus **katu** ja paina **Toista**. Kahdeksan sekunnin aikana
   Ilona vilkuttaa ja hätkähtää, Topi kääntyy sivuttain, nyökkää kahdesti ja
   kohauttaa olkiaan, ja Rauno ristii kätensä, kääntää selkänsä ja palaa
   takaisin.
3. `characters/` sisältää jokaisen hahmon omana projektinaan.
   **Model sheet ja hahmotyyli** -paneeli piirtää niistä kääntökuvan, ilmeet,
   suun muodot, päänkorkeusviivaimen ja siluettitarkistuksen.

## Hahmot

| Hahmo | Luonne ja tunnusmerkki |
| --- | --- |
| **Ilona** | Sanavalmis koululainen, joka keksii aina suunnitelman. Ruskea polkkatukka, keltainen tähtisolki ja turkoosi neule. |
| **Topi** | Innokas luokkakaveri, joka sanoo ensin ja ajattelee sitten. Violetti lippalakki, sinappinen huppari ja pisamat. |
| **Rauno** | Koulun talonmies, joka on nähnyt kaiken kahdesti. Kalju, viikset ja haalarit. |

Hahmot ovat noin kaksi päänkorkeutta pitkiä (Rauno vähän pidempi). Jokaisella
on 15 niveltä samoilla nimillä kuin tikku-ukkostudion hahmoilla, joten samat
eleet (`gesture`), kävelyt (`move`) ja nivelklipit toimivat. Mukana ovat 13
suun visemiä, katse, räpäytys sekä ilmeet `happy`, `sad`, `surprised` ja
`angry`. Visemit ovat käytettävissä puheen animointiin; äänetön
malliprojekti ei todista huulisynkan laatua.

## Paperileikkoliike

Jokaisella hahmolla on `timing: {on: 2, turn: snap}`:

- `on: 2` pitää jokaisen asennon kaksi ruutua (animointi kakkosilla), joten
  liike nykii hieman kuten paperinukeissa. Kamera ja kohtauksen kello pysyvät
  ykkösillä.
- `turn: snap` vaihtaa näkymän (edestä, puolisivulta, sivulta, takaa)
  heti `face`-tapahtuman ruudulla ilman pehmeää kapenemista.

Hiha on yksi saumaton pala olkapäästä ranteeseen. Se on sidottu sekä olka- että
kyynärniveleen (kahden nivelen skinning), joten käsi taipuu kyynärpäästä ilman
saumaa. Lahje taipuu samalla tavalla polvesta. Lapaset ja kengät ovat erillisiä
paloja.

Asettelu noudattaa paperinukkeanimaation käytäntöjä:

- Hihan olkapään pää on puoliympyrä nivelen ympärillä, joten kulma ei pilkistä
  vartalon reunasta käden kääntyessä.
- Lepoasennossa kädet roikkuvat rentoina vartalon vieressä, kyynärpäät hieman
  koukussa. Puolet eroavat hieman, jotta asento ei ole peilikuva.
- Kädet piirretään pään päälle, ja olkanivelen rajat (`joint_limits`)
  pysäyttävät nostetun käden noin 30 astetta vaakatason yläpuolelle. Isopäinen
  hahmo vilkuttaa siksi pään vieressä eikä käsi katoa pään taakse.
  Kyynärvarren rajat pitävät ristityt kädet rinnalla eikä leuan alla.
- Käännöksissä kengät osoittavat katsomissuuntaan.
- Vaatteet istuvat paikoillaan: paidoilla on suorat olkapäät ja vain
  pienesti pyöristetyt kulmat, olkanivel on vartalon kulmassa, ja hihansuut
  ovat kyynärvarressa juuri lapasen yläpuolella. Ilonan neuleessa on valkoinen
  kaulus ja resorihelma. Topin hupparissa on niskaan laskettu huppu,
  kengurutasku, nauhat ja resorihelma. Raunon paidassa on terävä kaulus
  henkselien alla, ja haalarin housuosa peittää paidan helman ja jakautuu
  lahkeiksi jalkojen välissä. Lapasissa on peukalo vartalon puolella.
- Eleet tukevat tunnetta: Rauno ristii kätensä vihaisena, Ilonan
  hätkähdys on ennakoiva kyykky, ponnahdus ja alastulo, ja Topi kohauttaa
  olkiaan surullisena.

Hiuksilla, lakeilla, viiksillä ja vaatteiden yksityiskohdilla on omat
piirroksensa jokaiseen näkymään (`views`). Silmät, kulmakarvat ja suu
kääntyvät riggin omalla kasvojen käännöllä ja piiloutuvat takaa katsottuna.
Topin nyökkäys on tehty `hold`-avaimilla: pää hyppää kahden asennon välillä
ilman välivaiheita.

Sama ilme sopii myös muille hahmoille: model sheet -paneelin tyyli `cutout`
antaa minkä tahansa riggin jokaiselle palalle oman paksun ääriviivan ja
paperivärit. Tyyli vaikuttaa toistaiseksi vain model sheetiin; käsikirjoitus
ei vielä tue `style:`-avainta.

## Tekijyys

Hahmot ja tausta ovat omaa, muokattavaa geometriaa. Niissä ei ole kolmannen
osapuolen kuvia eikä minkään olemassa olevan sarjan hahmoja. Tiedostot
tuotetaan skriptillä `scripts/generate_cutout_cast.py`; aja se uudelleen
muutosten jälkeen. Testi `crates/motion-export/tests/cutout_cast.rs` validoi
riggit, model sheetit, kakkosilla animoinnin ja välittömät käännöt.
Ympäristömuuttuja `CUTOUT_OUT=<kansio>` tallentaa testistä model sheetit ja
demoruudut PNG-kuviksi.

Kuvat on tarkistettu visuaalisesti kehityksen aikana testin PNG-tulosteista.
Sovellusta ei ole ajettu fyysisellä Macilla tämän paketin kanssa.
