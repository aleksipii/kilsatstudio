# Agentti 04 — alkuperäiset hahmot, aineistot ja visuaalinen jatkuvuus

Tila: **suunnitelma, ei toteutettu aineistovalitsin tai hyväksytty laatutaso**.
Tämä osuus muuntaa `ScriptIntent v1` -tarpeet versioiduksi `AssetManifest v1`
-valinnaksi. Sen jälkeen olemassa oleva MotionScript vastaanottaa tuetut rigien,
kuvien ja äänten viitteet. Uutta rinnakkaista renderöijää ei tehdä.

## Rajattu ensimmäinen tuotantoprofiili

30–60 sekuntia, enintään kaksi ennalta valmisteltua hahmoa ja kolme kohtausta.
Tavoite on alkuperäinen, viimeistelty tyylitelty 2D-animaatio: luettavat siluetit,
hillitty paletti, yhtenäiset mittasuhteet ja ilmeikkäät, käsin valmistellut osat.
Pixarin ja Disneyn tuotantojen käsityö voi kuvata laatutoivetta; niiden hahmoja,
aineistoja tai tunnistettavia malleja ei kopioida eikä vastaavaa laatua luvata.
Mielivaltaisen 3D-hahmon luonti, riggaus, simulointi ja renderöinti viidessä
minuutissa ei kuulu ensimmäisen profiilin sopimukseen.

Alkuperäisen Aino–Rane-pilotin **suunniteltu** tyylipaketti:

| Vakaa tunnus | Alkuperäinen ulkoasu | Jatkuvuuden lukitus |
| --- | --- | --- |
| `AINO` | Oranssi työliivi, tumma nuttura, vakaa avoin siluetti | Sama asu, mittasuhteet ja nivelkartta kaikissa kuvissa |
| `RANE` | Sininen takki, kapea solmio, terävämpi hartialinja | Sama asu ja oma ilmekirjasto; ilkeys näkyy toiminnassa |
| `office_v1` | Pelkistetty suomalainen huoltotoimiston vastaanotto | Ovi, pöytä ja valon suunta sidotaan näyttämökoordinaatteihin |
| `phone_aino` | Neutraali alkuperäinen puhelinprop | Omistaja, käsi ja näkyvyys seuraavat tapahtumahistoriaa |
| `kilsat_insert_v1` | Käyttäjän oikea hyväksytty sovelluskuva | Oikea kuva tarvitaan julkaisuun; luonnos merkitään havainnekuvaksi |

Nämä ovat valmistettavia määrittelyjä, eivät ilmoitus jo olemassa olevista
sarjahahmoista tai oikeista sovelluskuvista.

## Sisään- ja ulostulo

Sisään: `ScriptIntent.version`, `seed`, hahmojen vakaat tunnukset, tyylipaketin
tunnus, kohtauspaikat, prop-tarpeet ja aikavälit. Epäsuora kuvaus kuten
”hienompi takki” ei vaihda hahmomallia. Se palautuu käyttäjälle ratkaistavaksi.

`AssetManifest v1` sisältää `version`, `style_pack_id`, `style_pack_revision`,
`script_digest`, `rules_revision` ja tunnuksen mukaan järjestetyn `assets`-listan.
Jokaisella tietueella on vähintään:

- `id`, `kind`, `project_relative_path`, `sha256`, `byte_length`, `revision`;
- `capabilities` kuten `joint_tracks`, `viseme_variants`, `gaze` tai `alpha_image`;
- rigille `rig_schema_version`, `rig_digest` ja semanttisten nivelten kartta;
- kuvalle pikselimitat ja puretun muistin yläraja; äänelle näytemäärä ja formaatti;
- `origin`, `license_expression`, `license_text_digest`, `review_state` ja
  `review_evidence_ref`; `review_state` on `unreviewed`, `needs_changes` tai `approved`;
- `uses`: kohtaus-, hahmo- ja prop-viitteet sekä näkyvyyden aikavälit.

Provenienssi kertoo alkuperän; pelkkä tekijän nimi, SPDX-tunnus tai onnistunut
hash-tarkistus ei ole oikeuksien hyväksyntä. Julkaisulle tarvitaan kyseisen
revision ja käyttötavan kattava kirjattu tarkistus. Referenssikuvaa ei käsitellä
automaattisesti uudelleenjaeltavana aineistona. Puuttuva lisenssiselvitys estää
julkaisun, mutta ei erillisen virheraportin tuottamista.

## Deterministiset valintasäännöt

Kaikki `P0`-ehdot tarkistetaan ensin. Niitä ei ohiteta nopeustavoitteen vuoksi.
Muut ehdokkaat järjestetään prioriteetin, täsmällisen yhteensopivuuden ja lopuksi
vakaan `asset.id`-tunnuksen mukaan. Satunnaisuutta saa käyttää vain hyväksytyn
ilmevarianttijoukon sisällä `seed`-arvosta; se ei muuta oikeuksia tai rigikarttaa.

| Sääntö | Ehto ja toiminta | Virhe / testi |
| --- | --- | --- |
| `AST-001` P0 | Hylkää tuntematon manifestiversio, absoluuttinen polku, `..`, symlinkki projektin ulkopuolelle ja verkkoviite. Avaa rajattu paikallinen tavujono. | `asset_reference_invalid`; polun karkaus ei avaa ulkopuolista tiedostoa. |
| `AST-002` P0 | Tarkista revision tavumäärä ja SHA-256 ennen käyttöä; muuttunut sisältö mitätöi aiemman johdannaisen ja tarkistuksen. | `asset_identity_changed`; yhden tavun muutos tuottaa välimuistihudin. |
| `AST-003` P0 | Julkaisuprofiili vaatii jokaiselta aineistolta `approved`-tilan ja tarkistusviitteen; hyväksymättömiä resursseja ei pakata vientiin. | `asset_rights_unresolved`; lisenssikentän täyttäminen yksin ei läpäise porttia. |
| `AST-004` P0 | Tarkista vaaditut kyvykkyydet ja rigikartan täsmällinen yhteensopivuus ennen animaatiota. | `asset_capability_missing`; puuttuva suuvariantti ei muutu hiljaisesti staattiseksi suuksi. |
| `AST-010` P1 | Käyttäjän täsmällisesti valitsema hyväksytty revisio voittaa kaikki automaattiset ehdotukset. | Kaksi ajoa käyttää samaa rigihashia. |
| `AST-020` P2 | Ellei valintaa ole, käytä tyylipaketin hahmo-/paikkasidontaa, jonka kyvykkyydet täyttyvät. | Puuttuva pakettisidonta tuottaa ratkaistavan valinnan, ei keksittyä tiedostoa. |
| `AST-030` P3 | Saman paketin hyväksytty korvaava prop sallitaan vain etukäteen määritellyllä vastaavuudella ja näkyvällä korvausraportilla. | Juonelle olennainen puhelin ei vaihdu mukiksi. |
| `AST-040` P1 | Lukitse hahmon revisio ja paletti koko jaksolle; propin omistaja/kiinnitys muuttuu vain käsikirjoituksen tapahtumasta. | Leikkaus tai takaperin seek ei siirrä puhelinta toiselle hahmolle. |
| `AST-050` P0 | Laske puretut kuvat, rigiosat ja johdannaiset yhteiseen työbudjettiin ennen työn käynnistystä. | Liian suuri kuva ei purkaudu budjettilaskennan ohi. |

Täsmällinen ensiprofiilin lisärajaehdotus: 128 manifestitietuetta, 256 MiB
aineistotavuja, 512 MiB kuvien purettua muistia, yhden kuvan enimmäiskoko
4096×4096 ja 256 MiB yhdelle johdannaistyölle. Nämä ovat ehdotuksia; renderöijän
ja prosessin lopullinen yhteinen muistibudjetti on määritettävä integraatiossa.
Rigin osien/nivelten rajaksi käytetään nykyisen `motion-rig`-validaattorin rajaa,
ei arvaamalla annettua rinnakkaisrajaa. Kerto-/summalaskenta tarkistetaan ennen
allokointia, kaikki ajat/mitat ovat äärellisiä ja työn peruutus vapauttaa varaukset.

## Nykyisestä koodista todettu pohja ja avoimet kohdat

Tarkastetussa `crates/motion-script/src/lib.rs`-tiedostossa `Character` sisältää
`asset`-viitteen ja valinnaisen `motion_rig::RigDefinition`-rigin. `Composition`
sisältää `joint_tracks`- ja `gaze_tracks`-raidat; `RigClip`, `capture_rig_clip`
ja `apply_rig_clip` tarjoavat projektikohtaisia kopioitavia nivelraitoja sekä
nimenomaisen nivelkartoituksen. `Script::validate` rajoittaa rigikirjaston
64 klippiin ja 65536 avaimeen. Tämä on käyttökelpoinen pohja hyväksytyille
valmisrigeille; automaattista aineistovalitsinta se ei vielä osoita.

`examples/acting-rig.yaml` näyttää alkuperäisen polygonihahmon, suu- ja
kulmavariantit, visemekartan, katsekohteet ja räpäytysmäärityksen.
`examples/night-room-reference/README.md` kuvaa alkuperäisen litteän
cut-out-esimerkin ja sen paikkamerkkejä. Esimerkin lukeminen ei todista
suomalaista puhetta, teksturoitua deformaatiota, 3D-valaistusta tai tuotantolaatua.
Yleinen teksturoitu deformaatio, automaattinen oikeustarkistus, manifestin
valitsin ja Aino–Rane-tyylipaketti jäävät tämän suunnitelman toteutustöiksi.

## Viiden minuutin ehto ja hyväksyntä

Nopea ajo olettaa paikallisesti valmiit, hyväksytyt ja oikeilla hasheilla löytyvät
rigit, taustat, propit ja fontit. Uuden aineiston suunnittelu, maalaus, mallinnus,
lataus tai lisenssiselvitys ei mahdu luvattuun ajolupaukseen; niiden puuttuminen
palauttaa esitarkistusvirheen ennen renderöintiä. Kylmä ja lämmin aineistovälimuisti
mitataan erikseen nimetyllä Macilla. Aineistovalinnan alustava osabudjetti on
10 sekuntia, joka ei ole mitattu tulos eikä yksin takaa 300 sekunnin kokonaisuutta.

Hyväksyntään tarvitaan sääntötaulukon virhetestit, muuttumaton manifesti samalla
syötteellä/siemenellä, sekventiaalisen ja satunnaisen seekin sama prop-tila,
oikean tuetun rigin ajo yhteisen validaattorin läpi sekä riippumaton kuvallinen
tarkistus Aino–Rane-pilotin silueteista, värijatkuvuudesta ja prop-kiinnityksistä.
Kaikki nämä ovat suunniteltuja tarkistuksia, eivät tässä dokumentissa ajettuja testejä.
