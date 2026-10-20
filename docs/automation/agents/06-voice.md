# 06 — Puhe, suunliikkeet ja äänen kello

Tila: **suunnitelma**, ei uusi toteutus eikä suomenkielisen puheen laatuhyväksyntä.
Tarkastettu lähde: `7bb5e21331a672b15be5b2227c93ccc8171e8fa7`.
Omistaja: ääniagentti, yksi kymmenestä suunnitteluagentista.

## Nykyinen pohja ja täsmälliset rajat

- `crates/motion-script/src/lib.rs`: `VoiceConfig`, `Event::Speak`,
  `CaptionStyle`, `CaptionPreference`, `AudioCue` ja `Script::validate`.
  Repliikki vaatii hahmon ääniasetuksen tai tuodun äänen. Tekstityksen puhujavärit
  ja käyttäjän Show/Hide/Follow-valinta ovat jo olemassa. Säilytetään sama
  tekstityssuunnitelma esikatselussa ja viennissä.
- `crates/motion-voice/src/lib.rs`: `dialogue_jobs` antaa työt kohtaus- ja
  tapahtumajärjestyksessä. `VoiceWorker` käyttää yhtä työntekijää, rajattuja jonoja
  ja `CancellationToken`-peruutusta. Tuonti hyväksyy vain epätyhjän 24 kHz:n
  mono PCM16 WAVin; dekoodausvirhe epäonnistuu selvästi.
- `crates/motion-tts/src/lib.rs`: `SpeechProvider`, `SpeechRequest::validate`,
  `KokoroProvider`, `SpeechResult`. Nykyinen pyyntö hyväksyy **vain `en-us` ja
  `en-gb`**. Suomenkielistä Kokoro-ääntä ei voi tämän toteutuksen perusteella
  luvata. `VoiceSettings.style` on välimuistin metadataa, ei todistettu
  akustinen tunneohjaus.
- Samassa tiedostossa Kokoro-välimuistin avain sisältää pyynnön, asennuksen
  `model_version`-tunnisteen sekä malliasetuksen, painojen, äänen ja runtime-
  tiedoston sisällön tiivisteet. WAV tarkastetaan ja sen tiivistettä verrataan.
  Asentajan antaman ympäristötunnisteen täydellisyys vaatii oman tarkastuksen.
- `crates/motion-voice/src/timeline.rs`: `DialogueClip::prepare`,
  `prepare_with_alignment_input`, `derive_text_estimate`, `mouth_source`,
  `mouth_at` ja `mix_clip`. Jo toteutettu suulähteiden etusija on
  **manuaalinen → forced-aligned → Rhubarb-arvio → tekstiarvio**.
  Englannin tekstiarvio käyttää `EnglishError`-rajapintaa; siitä ei tehdä
  suomenkielisen huulisynkan laatutodistetta.
- `crates/motion-lipsync/src/lib.rs`: `AlignmentSource`, `Alignment::validate`,
  `visemes` ja `viseme_at` erottavat ajoituksen alkuperän. Aikaleimojen
  rakenteellinen kelvollisuus ei mittaa katsottua tai kuunneltua laatua.

## Ehdotettu AudioPlan v1 -sopimus

Syöte on `DirectionPlan v1`, hahmojen valitut hyväksytyt ääniresurssit,
projektin juurihakemisto, tämän ajon jäljellä oleva määräaika ja
sisältötiivisteillä yksilöity paikallinen välimuisti. Tulos lasketaan
**samaan olemassa olevaan MotionScript/audio/timeline-putkeen**. Tässä ei
ehdoteta rinnakkaista renderöijää eikä toista näytekelloa.

`AudioPlan` sisältää version, lähteen tiivisteen, sääntöversion, 24000 Hz:n
projektikellon sekä enintään kahden hahmon enintään 60 sekunnin klipit.
Jokaisella klipillä on vakaa `line_id`, `actor_id`, alkuperäinen teksti ja
kieli, lähdetekstin kohta, `start_sample`, `sample_count`, trimmaus, gain,
PCM-tiiviste, tuonnin tai providerin identiteetti, puheen oikeusnäyttö,
suulähteen provenienssi ja tekstityksen puhujatunniste.

Puuttuva PCM merkitään `pending`, ei keksityllä nollanäytteellä valmiiksi.
Puheen tila, ajoituksen alkuperä ja laadun tila ovat erillisiä kenttiä:

| Kenttä | Sallitut merkitykset |
| --- | --- |
| `audio_state` | `pending`, `ready`, `failed` |
| `mouth_source` | `none`, `manual`, `forced`, `recognizer_estimated`, `text_estimated` |
| `quality_state` | `not_reviewed`, `reviewed_pass`, `reviewed_fail` |

`reviewed_pass` vaatii todellisen ääni-/videotarkastuksen tiedot; se ei synny
pelkästä `Alignment::validate`-kutsusta. Valmis äänellinen vienti edellyttää
kaikkien repliikkien todellista PCM:ää. Hiljainen esikatselu on erillinen,
käyttäjän nimeämä luonnostuote.

## Säännöt ja deterministinen ratkaisujärjestys

Etusija on pienempi numero ensin. Samalla tasolla ratkaistaan `line_id`:n
mukaan, ei työntekijän valmistumisjärjestyksen perusteella. Täsmällinen
käyttäjän muokkaus ohittaa automaattisen ehdotuksen, mutta ei formaatin,
resurssien, puheoikeuksien tai määräajan kelpoisuustarkastusta. Äänivalinnassa
ei ole satunnaisuutta; yhteinen seed ei vaihda hahmon ääntä.

| Tunnus / etusija | Ehto ja toiminta | Virhe tai näyttö |
| --- | --- | --- |
| VOICE-001 / 0 | Tarkasta kieli, provider, paikalliset malli-/äänitiedostot ja oikeusnäyttö ennen synteesiä. FI-pilotissa valitse tallennettu suomalainen ääni tai erikseen varmennettu FI-provider. | `VOICE_LANGUAGE_UNSUPPORTED` tai `VOICE_ASSET_MISSING`; ei englannin ääntä hiljaisena korvikkeena. |
| VOICE-002 / 0 | Syötteen käyttäjän valitsema tuonti ohittaa synteesin. Tarkasta säännöllinen tiedosto projektin hyväksytyssä asset-juuressa, kokoraja, koko WAV ja PCM-sopimus ennen julkaisua. | `VOICE_AUDIO_INVALID`; lähdetiedoston vaihto mitätöi suunnitelman tiivisteen. Asset-juuren lisäraja on ehdotus. |
| VOICE-003 / 10 | Hae vain täsmälleen vastaava puheavain. Sisällytä teksti, kieli, ääni, nopeus, provider/model/runtime/ympäristöidentiteetti ja kaikki tulokseen vaikuttavat parametrit. Tarkasta PCM ennen käyttöä. | Korruptoitunut osuma on miss, ei onnistuminen. Älä muuta käsikirjoitusta välimuistiosuman saamiseksi. |
| VOICE-004 / 20 | Käytä olemassa olevaa rajattua VoiceWorker-jonoa. Välitä ajon peruutus ja työkohtainen timeout. Älä aloita työtä, jonka mitattu yläraja ylittää jäljellä olevan ajan. | `VOICE_DEADLINE_UNFEASIBLE`; ei raskasta työtä UI-säikeelle. Ylärajan mittaaminen on avoin hyväksyntä. |
| VOICE-005 / 30 | Mittaa valmis ääni näytteistä. `end = start + sample_count - trim`; kaikki laskut tarkastetuin kokonaisluvuin. | `DIALOGUE_DOES_NOT_FIT` nimeää repliikin ja puuttuvat näytteet. Ei automaattista tekstin leikkausta tai nopeutusta. |
| VOICE-006 / 40 | Pidä dialogi ehjänä. Sijoita vapaaseen loppuaikaan ehdotuksena 0,18 s vastaustauko ja 0,35 s koominen reaktio; käyttäjän tauko voittaa. Sovita näyttely ja otos vasta mitattuun puheeseen. | Tauko on taiteellinen oletus, ei puheen kestomittaus. Jos 30–60 s raja ylittyy, pyydä lyhentämään tai valitsemaan pidempi profiili. |
| VOICE-007 / 50 | Valitse olemassa oleva manuaalinen/forced/recognizer/text-etusija. Tarkasta PCM- ja tekstitiiviste, näytemäärä sekä tunnistin-/sanastoidentiteetti ennen arvioiden uudelleenkäyttöä. | Väärän äänen tai vanhan tekstin huuliraita hylätään. FI ei automaattisesti saa englannin tekstiarviota hyväksytyksi huulisynkaksi. |
| VOICE-008 / 60 | Laadi tekstitykset samoista repliikeistä ja klippien näyteajoista. Kunnioita CaptionPreference-valintaa. Puhujaväri ei yksin ilmaise puhujaa; tarkista tekstin luettavuus lopullisessa koossa. | `CAPTION_OVERFLOW` tai `CAPTION_UNREADABLE`; ei mielivaltaista dialogin lyhennystä. |
| VOICE-009 / 70 | Sekoita repliikit ja erilliset AudioCue-äänet samalla 24 kHz kellolla. Pidä puhe etualalla; ehdotettu sivuäänen vaimennus on eksplisiittinen asetettava sääntö. | Leikkautumisen mittaus ja kuuntelu vaaditaan. Lopullinen loudness-raja päätetään ja mitataan pilotissa. |
| VOICE-010 / 80 | Julkaise äänellinen tulos vain, jos kaikki puheklipit, synkronointi, tiedostot ja vienti ovat valmiit. | `AUDIO_DELIVERY_INCOMPLETE`; palautetaan tarkka raportti ja muokattava keskeneräinen projekti. |

## Viiden minuutin rajaus

Äänen osuudelle ehdotetaan **enintään 60 sekunnin** seinäkelloikkunaa
300 sekunnin kokonaisajon sisällä; tämä ei ole nykyinen benchmark eikä
vaiheiden aikojen summalla taattu SLA. Sisältörajana on yksi 30–60 sekunnin
jakso, kaksi jo hyväksyttyä ääntä ja valmiiksi asennetut mallit. Täydellinen
välimuistiosuma on oma mitattava tapaus. Uusi FI-synteesi on erillinen
mitattava tapaus, ei sama lupaus. Mallien lataus, uuden äänen koulutus,
studioäänitys ja kielituen rakentaminen eivät kuulu lämpimän ajon profiiliin.

Kohtauspaikkojen alustava sanamääräarvio on vain preflight-varoitus.
Viidenkymmenen sekunnin käsikirjoitus ei todista, että luettu teksti kestää
50 sekuntia. Ennen lopullista otoslukitusta vaaditaan oikea näytemäärä.
Määräajan ylittyminen ei muuta keskeneräistä tulosta onnistumiseksi.

## Hyväksyntäesimerkit, joita pitää toteuttaa ja ajaa

1. FI-repliikki + nykyinen KokoroProvider ilman tuontia: selvä kielivirhe,
   ei verkkoa, ei englanniksi lausuttua tiedostoa valmiina FI-puheena.
2. Sama FI-repliikki + oikea hyväksytty PCM16/24 kHz WAV: todellinen
   näytemäärä määrää klipin lopun. Stereo, 48 kHz, tyhjä tai katkennut WAV
   hylätään; tuontipolku ei kierrä lopullisia puheoikeustarkastuksia.
3. Sama teksti mutta vaihdettu ääni/runtime/malli tai PCM: vanhaa puhetta
   tai huulisynkkaa ei käytetä. Korruptoitunut osuma tuottaa missin.
4. Esimerkiksi 720000 näytteen repliikki 600000 näytteen ikkunassa:
   120000 näytteen ylitys näkyy virheessä; tekstiä ei typistetä.
5. Manuaalinen raita säilyy tekstiarvion ja tunnistintuloksen saapuessa.
   Väärään PCM:ään sidottu manuaalinen raita vaatii korjauksen, ei ohitusta.
6. Seek/pause/kohtausraja ja peräkkäiset viennit: PCM, suunmuoto ja
   tekstityksen repliikki vastaavat samaa näyteindeksiä, myös ei-peräkkäisinä
   näytteinä. Toleranssi määritetään näytteissä ja kuvakehyksissä erikseen.
7. Peruutus tai 60 sekunnin äänibudjetti: ei keskeneräistä välimuistiosumaa
   eikä valmiiksi nimettyä hiljaista videota. Uusintayritys voi käyttää vain
   aiemmin eheästi julkaistuja klippejä.
8. Oikea suomalainen puhe, manuaalisesti annotoidut huulet ja Aino/Rane-
   pilotti: kuunneltu ymmärrettävyys, katsottu synkronointi ja kaksi erillistä
   hahmoääntä arvioidaan; kuuntelijan tulos ja mittausaineisto säilytetään.

Tämä suunnitelma tavoittelee hallittua ääni- ja näyttelylaatua. Se ei osoita
Pixar-/Disney-tasoa, suomenkielisen synteesin toimivuutta, viiden minuutin
suorituskykyä tai ääniresurssien lopullista jakelulupaa.
