# Tikku-ukkostudion kuvallinen suunnitelma

Tämä suunnitelma ohjaa tikku-ukkostudion toista sisältöversiota. Tavoite on
luettava, yhtenäinen satiirimaailma: ohuet mustat raajat, pyöreät kasvot,
hahmokohtainen tunnusväri ja pelkistetyt ympäristöt. Työ säilyy muokattavana
Kilsatin omana geometrisena sisältönä. Toteutuksen tarkistustulokset kirjataan
[etenemisseurantaan](master-progress.md); alla olevat ehdot eivät itsessään ole
testitodisteita.

## 1. Hahmo pysyy yhtenäisenä liikkeessä

Raajan tasapaksu viiva päättyy nivelen keskelle pyöristetyllä päädyllä.
Kyynärpään ja polven viivat menevät päädyistä päällekkäin myös taivutuksessa.
Lantion poikkiviiva yhdistää reidet alempaan selkärankaan; hartiaviiva yhdistää
käsivarret ylävartaloon. Alempi selkäranka kuuluu lantiolle ja ylempi vartalon
nivelelle, joten vartalon kierto ei irrota selkää lantiosta. Kaula ulottuu pään
sisään. Liitoskohtaa ei piiloteta vaatteen tai taustan avulla.

Hahmojen nykyiset 15 niveltä, kasvokartat ja tunnusmerkit säilyvät. Päsmän
lehtiö, Kiireen paketti, Sisun keppi ja Noutajan viikate liittyvät kämmenen
kohdalle. Tasapaksut varret säilyttävät tikku-ukkotyylin myös lähikuvassa.

## 2. Käsi todella pitää esinettä

Yhdeksän käteen sopivaa vaihtoehtoa ovat puhelimen näyttö-, taka- ja vinonäkymä,
kahvikuppi, kirja, avain, tabletti, kynä ja työkaluavain. Esineen tartuntapiste
on käden nivelen kohdalla; sormi- ja peukalomuodot tulevat esineen eteen.
Puhelimen reunus, näyttö ja kamerat erottavat kolme kuvallista näkymää.
Koko vastaa kämmentä, eikä oletuspuhelin peitä koko päätä.

**Esine käteen** käyttää hahmon olemassa olevaa rigiä. Käden, kyynärvarren,
olkapään ja vartalon liike sekä hahmon mittakaava kuljettavat esinettä samalla
arvioinnilla esikatselussa ja viennissä. Sijaintia, kiertoa ja kokoa voi säätää.
Kiinnitys koskee valitun hahmon kaikkia kohtauksia; ajastettu poimiminen tai
irrottaminen ei kuulu tähän vaiheeseen. Pysyvästi varattuun käteen valitaan
toinen käsi, jotta hahmon tunnusesine säilyy.

## 3. Tausta tukee hahmoa

Neljä ympäristöä säilyvät: toimisto, bussipysäkki, hautausmaa ja kahvila.
Taustan suuret pinnat ovat hillittyjä ja viivat hahmon siluettia rauhallisempia.
Etuala, lattia ja takaseinä tai kaukoinen maisema antavat syvyyden. Kalusteet
muodostavat selkeitä ryhmiä: niiden koko ja sijainti kuuluvat tilaan.

Malliprojektin jalkojen kosketuskorkeus on noin y=960 Full HD -pohjalla.
Lattian takareuna jää ylemmäs, jolloin hahmo seisoo lattialla eikä seinän
reunaviivalla. Yömaisemien yhtenäinen siniharmaa sävy erottaa tummat raajat;
suuri harmaa soikio poistuu. Irrallisia pienoisrekvisiittoja ei ripotella
kuvan reunaan täyttämään tyhjää tilaa.

## 4. Kuvakoolla on tarkoitus

| Kuva | Käyttö |
| --- | --- |
| Yleiskuva | Näytä paikka, molemmat hahmot ja jalkakontakti. |
| Hahmot samaan kuvaan | Näytä keskustelu ja hahmojen välinen etäisyys. |
| Puolikuva | Näytä käsiele, esine ja ylävartalon näyttely. |
| Lähikuva | Korosta katsetta, ilmettä tai vitsin jälkeistä reaktiota. |
| Kädessä oleva esine | Tee kädessä olevasta esineestä tarinan kannalta luettava lähikuvassa. |

Kuvakokotyökalu kirjoittaa olemassa olevan kameran siirto- ja zoomiavaimen
toistopään kohtaan. Rajaus seuraa arvioitua hahmoa tai kädessä olevan esineen
todellista geometriaa. Kamera on 2D-kamera: puhelimen taka- tai vinonäkymä
valitaan eri esineenä. Kolmiulotteista kiertorataa ei synny zoomaamalla.

Kuuden kohtauksen 30 sekunnin malliprojekti vaihtelee yleiskuvaa, kahden hahmon
kuvaa ja lähempiä kuvia. Puhelimen käyttö, kahvikupin pito ja työkalun liike
antavat kuvakoolle näkyvän tarkoituksen. Malliprojekti pysyy äänettömänä;
puheen tai koomisen rytmin laatua ei päätellä siitä.

## 5. Tarkistettava ennen toimitusta

1. Renderöi jokainen kuudesta hahmosta seitsemässä asennossa: lepo sekä pään,
   vartalon, käsien ja jalkojen taivutukset. Tarkista myös nivelten lähikuvat.
   Läpinäkyvän rasterin merkittävien osien on oltava yhteydessä toisiinsa;
   testi ei saa yhdistää aukkoja laajentamalla kuvapisteitä.
2. Tarkista puhelimen käteen sidonta ranteen, kyynärpään, olkapään ja vartalon
   kierroissa sekä koko hahmon kierrossa ja epätasaisessa skaalauksessa.
   Vertaa natiivin esikatselun ja vientikuvan rasteria.
3. Tarkista kaikkien neljän taustan natiivit kuvat ja kaikki kuusi videokuvaa:
   siluetti, jalkakontakti, rekvisiitan koko, taustan perspektiivi ja rajaus.
   Tekninen liitostesti ei korvaa tätä katselmointia.
4. Tarkista oikean Full HD -MP4:n kuvamäärä, kesto ja dekoodaus sekä äänettömän
   esittelyn nollasamplet. Kuvapakettiin kuuluvat 30 natiivia sisältöprojektia,
   60 PNG/SVG-kuvaa, käsirekvisiitan luettelo ja malliprojekti.
5. Käy Macissa läpi kiinnitä/vaihda/poista, kumoa/toista, kuvakoon valinta,
   tallennus ja uudelleenavaus. Kirjaa laite, macOS ja havainnot erikseen.

Seuraavat askeleet ovat visuaalinen nivel- ja IK-muokkaus, lattiaan lukittuva
jalka, esineen ajastetut kontaktit sekä puhutun kohtauksen taiteellinen arvio.
Nykyinen liitoskorjaus ei yksin ratkaise näitä. Developer ID, notarisointi ja
lopullinen oikeustarkistus säilyvät erillisinä julkaisuehtoina.
