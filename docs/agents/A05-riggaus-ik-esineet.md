# A05 — Riggaus, IK ja esineet

Tila: **done** (G2 KS-003a, PR #117 `824c399`; IK-aikajanaraita KS-003d:ssä) · Portti: G2 · Tehtävä: KS-003 · Lähtö: commit `496a8a4`

## Tavoite

Säilyttää tikku-ukkotyyli; kiinnittää jalat lantioon ja kädet hartioihin; toteuttaa
nivelrajat, deterministinen kahden luun IK, käden tartuntapisteet ja puhelin, joka
pysyy kädessä hahmon liikkuessa ja kameran vaihtuessa.

## Lähtötiedot

- `crates/motion-rig/src/lib.rs` (`Transform2`, `Affine2`, `Rig::evaluate`),
  `definition.rs` (custom rig, rigid parts, attachment bind, `evaluate_acting`)
- `crates/motion-animation/src/clips.rs` (`evaluate_humanoid`, 7 pistettä)
- `apps/studio-desktop/src/hand_props.rs` (`grip`, `grip_parts`, esineluettelo)
- `crates/motion-export/examples/stick_studio_assets.rs`, `examples/stick-studio/`
- [custom-rigid-characters.md](../custom-rigid-characters.md), [rig-outlines.md](../rig-outlines.md),
  [stick-studio-art-direction.md](../stick-studio-art-direction.md)

## Nykytila (koodista)

- Luuhierarkia, rigid-osat ja kiinnitysten bind-matriisit ovat olemassa.
- Käsiesineillä on paikallinen `grip`-nivel, mutta esineluettelo on `studio-desktop`-sovelluksessa.
- IK-ratkaisijaa tai nivelrajoja ei löytynyt.
- Raportin SkelForm/PSD-tuonti ja GPU-verkkodeformaatio eivät ole nykyistä koodia.

## Sallittu muutosalue

Tarkka jako: [ORK-G2-01](00-orkestroija.md#ork-g2-01--g2n-tiedostoalueet-a05-a06-ja-a08-27102026). Tiivistelmä:

- `crates/motion-rig/**`; `crates/motion-animation/src/clips.rs` ja uusi
  `crates/motion-animation/tests/limb_attachment.rs` (ei `lib.rs`:ää eikä A06:n `spring.rs`:ää).
- `apps/studio-desktop/src/hand_props.rs` ja `examples/stick-studio/hand-props.json`
  esineluettelon siirtoon `motion-rig`iin (ORK-G1-05); `main.rs` ennallaan.
- `examples/g2-rig/**`, `docs/custom-rigid-characters.md`, `docs/rig-outlines.md`,
  uusi `docs/rig-attachment-ik.md`.
- IK-kohteen aikajanaraita: kirjoitettu muoto A02, ratkaisu A04 (KS-003d); A05 tarjoaa puhtaan funktion.
- Ei `rusty_skelform`-riippuvuutta ilman A01:n päätöstä.

## Riippuvuudet

- Edeltää: A01, A04 (aikajana).
- Sopii yhdessä: A06 (vedettävät raajat käyttävät IK:ta), A08 (renderöinti eri kuvakulmissa).

## Tehtävät — tarkistuslista

- [x] Kirjaa kiinnityssopimus: mitkä nivelet ovat juuria (lantio), mitkä lapsia
      (hartiat → kädet, lantio → jalat), ja varmista testillä, että raajat pysyvät kiinni
      kaikissa nykyisissä klipeissä.
- [x] Nivelrajat (min/max kulma) rig-määrittelyyn; validointi ja testit.
- [x] Analyyttinen kahden luun IK (lakikosinilla), taivutussuunta, saavuttamaton kohde
      → venytetty suora raaja ilman NaN-arvoja; testit rajoilla.
- [ ] IK-kohde aikajanaraidaksi tai tapahtumaksi A04:n sopimuksen mukaisesti.
      → Ei A05:n alueella (ORK-G2-01): IK on puhdas funktio, kohde arvona; raita on A02:n
      skeemapyyntö ja A04:n ratkaisu (KS-003d).
- [x] Tartuntapistesopimus: esineen `grip`-nivel sidotaan käden päätenivelen maailmamuunnokseen.
      Päätä esineluettelon omistava crate (ehdokas `motion-rig` tai `motion-art`).
      → `motion-rig::props` (ORK-G1-05).
- [x] Testi: puhelin pysyy kädessä (toleranssi pikseleinä) IK-liikkeessä, kävelyssä ja
      kameran vaihtuessa.
      → Kamera on testissä affiininen sijainen; renderöijän kuvat ovat A08:n (KS-003c).
- [x] Säilytä tikku-ukkotyyli; ei uutta piirtotyyliä ilman taiteellista päätöstä.

### ORK-LIB-01 — esinemanifestit (28.10.2026, haara `feat/asset-library`)

Alue: `examples/stick-studio/library/props/**`, `scripts/generate_library_props.py`, tämä tiedosto.

- [x] Generaattori `scripts/generate_library_props.py` (Python 3 stdlib, ei verkkoa): kirjoittaa
      `library/props/<id>.json` (indent 2, kiinteä avainjärjestys, rivinvaihto lopussa), poistaa vain
      vanhentuneet `prop.*.json`-tiedostot, `--check` vertaa levyyn.
- [x] Kaikki 20 `examples/stick-studio/props/*.kilsat`-esinettä migroitu `rig_project`-manifesteiksi
      (polku + sha256), bounds geometriasta, ankkurit (grip/ground/seat/screen/stand).
- [x] Kaikki 34 ORK:n pakollista art-esinettä tarkoilla tunnisteilla ankkureineen.
- [x] Lisää alkuperäisiä art-esineitä (keittiö, olohuone, toimisto, ulko, Suomi/talvi, kauppa, luokka).
- [x] Skriptin oma itsetarkistus (id, tagit, ankkurit ja geometria boundsissa, paint-slotit, sha256,
      viivat ≤ 8 px, lattialla seisovan alareuna = ground).
- [x] `validate_library --distribution` -ajo (ks. Toteutunut lopputulema).

## Hyväksymisehdot

- Raajat ja esine pysyvät kiinni hahmon liikkuessa ja kameran vaihtuessa (testit).
- IK tuottaa äärelliset arvot kaikilla syötteillä; samasta syötteestä sama tulos.
- Nykyiset rig- ja stick-studio-testit läpäisevät.

## Lopputulema

Odotettu: rig- ja kiinnityssopimus, nivelrajat, IK-ratkaisija, jaettu tartuntapistesopimus
ja kahden tikku-ukon testikohtaus puhelimen kanssa A08:lle.

### Toteutunut lopputulema

KS-003a, haara `feat/a05-rig-ik-props` (27.10.2026). Sopimus: [rig-attachment-ik.md](../rig-attachment-ik.md).

- **Kiinnitys** (`motion-rig/src/attachment.rs`): `PELVIS`/`TORSO`, `STICK_LIMBS`,
  `PreparedRig::stick_limbs`, `check_limb_offsets` (raajanivel saa vain kiertyä),
  `attachment_gap`. Testattu kaikilla kuudella stick-studio-hahmolla ja kaikilla
  `examples/stick-studio/**/*.kilsat`-niveliraidoilla (avainajat ja välipisteet); sisäänrakennettu
  7-pisteinen humanoidi `motion-animation/tests/limb_attachment.rs`:ssä (hierarkia, ei yli 2× venymää;
  suurin havaittu ≈ 1,76×).
- **Nivelrajat** (`definition.rs`): `RigDefinition::joint_limits` (`min_rotation`/`max_rotation`,
  rad), validointi (äärellinen, järjestys, ±2π, sisältää lepokulman, tunnettu nivel), puristus
  kaikissa arvioinneissa. Kulkee MotionScriptin `rig`-kentän läpi ilman `motion-script`-muutosta.
- **IK** (`ik.rs`): `solve_two_bone` (lakikosini, `BendDirection`, saavuttamaton → suora/taitettu raaja,
  `reached`), tyypitetyt `IkError`it; `PreparedRig::solve_limb_ik` rajojen kanssa (`limited`, `end`).
  Testit: rajat, yhtenevä piste, NaN/∞/liian suuret syötteet, ruudukko äärellisyydelle ja determinismille.
- **Esineet** (`props.rs`): esineluettelo, `GripPlacement`, `attach_hand_prop`/`detach_hand_prop`,
  `hand_availability`, `grip_parts` siirretty `apps/studio-desktop/src/hand_props.rs`:stä; työpöydän
  moduuli on ohut kääre (suomenkieliset virheet, `main.rs`:n rajapinta ennallaan).
  `motion-rig` sai `serde_json`-riippuvuuden (työtilan olemassa oleva riippuvuus, ei uusi ulkoinen
  crate) luettelon JSON-jäsennystä varten.
- **Testikohtaus A08:lle**: `examples/g2-rig/two-sticks-phone.json` (Veikko + Kipinä, puhelin oikeassa
  kädessä, IK-kohteet, kävely, laaja/lähikamera). `motion-rig/tests/g2_rig.rs` tarkistaa, että puhelimen
  pisteet pysyvät ≤ 0,5 px käden otepisteen muunnoksesta kaikissa asennoissa, askelissa ja kameroissa.
- Tarkistettu työtilan juuresta yhteisellä `CARGO_TARGET_DIR`illä `cargo clean -p motion-rig -p
  motion-animation -p studio-desktop` -ajon jälkeen: `cargo fmt --all --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets` (ei varoituksia), `cargo test --workspace` (exit 0; 48
  testisarjaa doc-testit mukaan lukien, 925 läpi, 0 virhettä, 8 ohitettu). Välissä toinen työpuu poisti
  `motion-render`in käännöksen yhteisestä kansiosta; korjattu `cargo clean -p motion-render` ja uusi ajo.
- Rajaukset: IK-aikajanaraita ja kohtausarviointi (KS-003d) puuttuvat; ei IK-editoria; kamera on testissä
  affiininen sijainen, ei renderöijän kuvaa; ei graafista tai Mac-laitetarkistusta.

#### ORK-LIB-01 — esinemanifestit (28.10.2026)

- **Määrät** (generaattorin laskema): 101 esinemanifestia = 20 migroitua `rig_project`-esinettä
  + 81 alkuperäistä art-esinettä (sis. kaikki 34 pakollista). Ankkurit: ground 75, grip 32, seat 13,
  screen 12, stand 10, doorway 1 (yht. 143). Seitsemällä seinäesineellä (ikkuna, kellot, verhot,
  peili, taulu, ryijy) ei ole ankkureita (kenttä jätetty pois).
- **Migraatio**: 9 `.kilsat`-tiedostoa on rigejä (puhelin ×3, kahvikuppi, kirja, avain, tabletti, kynä,
  kiintoavain): bounds rig-osista paikallisissa koordinaateissa, outline `behind` +leveys / `over`
  +leveys/2; grip (0, 0). 11 tiedostoa (auto, kannettava, kello, liikennemerkki, paketti, sateenvarjo,
  toimistotuoli, työpöytä, viikate, vuoronumerolaite, ajopäiväkirja) **ei sisällä rigiä** vaan
  art-kerroksia 512×512-pikkukuvan koordinaatistossa; bounds näistä kerroksista, ankkurit kerrosten
  nimien perusteella (esim. näyttö, istuin, varsi). Näiden mittakaava ei ole näyttelijämittakaava 1.
- **hand-props.json**: kaikkien 9 käsiesineen osat ovat täsmälleen samat kuin `.kilsat`-rigin osat
  (`grip`-nivel ↔ `root`, `grip: [0,0]`), joten koordinaatisto on sama: muunnos on identiteetti.
  Sen `width/height` on ääriviivaton laajuus; manifestien bounds sisältää ääriviivat.
- **Mittakaava**: hahmot 379–422 px pää–jalkapohja (scale 1), lantio 142 px, polvi ≈ 72 px, lepokäsi
  ≈ 181 px lattiasta. Istuimet 72–86 px, pöydät 118 px, työtasot 145 px, ovi 440 px.
- **Tarkistettu**: generaattori ajettu kahdesti ja `PYTHONHASHSEED=123`:lla → sama koostetiiviste;
  `python3 scripts/generate_library_props.py --check` → `ok: 101`; oma itsetarkistus ilman virheitä;
  `cargo run --locked -p motion-library --example validate_library -- examples/stick-studio
  --distribution` → exit 0, props 101, warnings 0. Art-esineet katsottu omalla SVG→PNG-kontaktiarkilla
  (perus- ja evening-paletti); ei sovelluksen renderöijän kuvaa.
- Rajaukset: ei taidekatselmointia renderöijällä; art-esineiden paletit eivät ole `check_background`-
  tarkistuksessa erikseen (sijainnit ovat A08:n).

## Kopioitava tehtävänanto

> Toimi agenttina A05. Lue `AGENTS.md`, `docs/agents/README.md` ja
> `docs/agents/A05-riggaus-ik-esineet.md`. Säilytä tikku-ukkotyyli. Tee tarkistuslistan
> tehtävät sallitulla alueella, aja työtilan tarkistukset ja päivitä tarkistuslista
> sekä Toteutunut lopputulema.
