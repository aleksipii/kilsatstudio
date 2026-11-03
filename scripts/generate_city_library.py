#!/usr/bin/env python3
"""Write the city props of `stick_city.py` as anchored library manifests.

Usage (from the workspace root): `python3 scripts/generate_city_library.py`

Each prop becomes `examples/stick-studio/library/props/<id>.json` (schema v1,
docs/asset-library.md): art geometry with the bounds origin at 0, a ground
anchor along the bottom edge and Finnish/English search tags. Street props and
vehicles are scaled to 120 px per metre (the library's actor scale 1); buildings
are 0.6 of their drawing size and are meant to be placed with a part `scale`.
Strokes are clamped to 18 px, the library line limit at actor scale 1.
Output is deterministic; the city set backgrounds come from
`crates/motion-library/examples/generate_stick_library.rs`.
"""
from __future__ import annotations

import json
from pathlib import Path

from stick_city import CITY_PROPS

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "examples/stick-studio/library/props"
PX_PER_METRE = 120.0
BUILDING_SCALE = 0.6
MAX_LINE = 18.0

# key: (library id, depth, real height in metres or None for a building, tags)
CATALOG = {
    "henkiloauto": ("prop.vehicle.sedan_v1", "mid", 1.45,
                    ["henkilöauto", "auto", "car", "sedan", "ajoneuvo", "vehicle", "katu", "street", "city"]),
    "taksi": ("prop.vehicle.taxi_v1", "mid", 1.55,
              ["taksi", "taxi", "auto", "car", "ajoneuvo", "vehicle", "katu", "street", "city"]),
    "poliisiauto": ("prop.vehicle.police_car_v1", "mid", 1.6,
                    ["poliisiauto", "poliisi", "police", "police_car", "auto", "car", "vehicle", "city"]),
    "pakettiauto": ("prop.vehicle.van_v1", "mid", 2.1,
                    ["pakettiauto", "van", "delivery", "auto", "ajoneuvo", "vehicle", "street", "city"]),
    "ambulanssi": ("prop.vehicle.ambulance_v1", "mid", 2.3,
                   ["ambulanssi", "ambulance", "sairasauto", "rescue", "pelastus", "vehicle", "city"]),
    "linja_auto": ("prop.vehicle.city_bus_v1", "mid", 3.0,
                   ["linja_auto", "bussi", "bus", "joukkoliikenne", "transit", "vehicle", "street", "city"]),
    "paloauto": ("prop.vehicle.fire_engine_v1", "mid", 3.2,
                 ["paloauto", "fire_engine", "fire_truck", "palokunta", "rescue", "pelastus", "vehicle", "city"]),
    "skootteri": ("prop.vehicle.scooter_v1", "mid", 1.15,
                  ["skootteri", "mopo", "scooter", "moped", "ajoneuvo", "vehicle", "street", "city"]),
    "liikennevalo": ("prop.street.traffic_light_v1", "mid", 3.4,
                     ["liikennevalo", "liikennevalot", "traffic_light", "risteys", "crossing", "street", "city"]),
    "paloposti": ("prop.street.fire_hydrant_v1", "mid", 0.8,
                  ["paloposti", "fire_hydrant", "hydrant", "katu", "street", "city", "outdoor"]),
    "pysakointimittari": ("prop.street.parking_meter_v1", "mid", 1.4,
                          ["pysäköintimittari", "parkkimittari", "parking_meter", "parking", "street", "city"]),
    "pankkiautomaatti": ("prop.street.atm_v1", "mid", 1.8,
                         ["pankkiautomaatti", "atm", "cash_machine", "pankki", "bank", "street", "city"]),
    "juoma_automaatti": ("prop.shop.vending_machine_v1", "mid", 1.9,
                         ["juoma_automaatti", "automaatti", "vending_machine", "juoma", "drink", "shop", "city"]),
    "katupuu": ("prop.outdoor.street_tree_v1", "mid", 6.0,
                ["katupuu", "puu", "tree", "street_tree", "puisto", "park", "street", "outdoor", "city"]),
    "puistonpenkki": ("prop.outdoor.park_bench_v1", "mid", 0.9,
                      ["puistonpenkki", "penkki", "bench", "park_bench", "puisto", "park", "outdoor", "city"]),
    "suojatiemerkki": ("prop.street.crosswalk_sign_v1", "mid", 2.6,
                       ["suojatiemerkki", "suojatie", "crosswalk", "liikennemerkki", "road_sign", "street", "city"]),
    "omakotitalo": ("prop.building.family_house_v1", "far", None,
                    ["omakotitalo", "talo", "house", "koti", "home", "building", "rakennus", "kiinteistö",
                     "property", "asuinalue", "residential"]),
    "kerrostalo": ("prop.building.apartment_block_v1", "far", None,
                   ["kerrostalo", "talo", "apartment", "apartments", "flats", "building", "rakennus",
                    "kiinteistö", "property", "city"]),
    "kulmakauppa": ("prop.building.corner_shop_v1", "far", None,
                    ["kulmakauppa", "kauppa", "shop", "store", "grocery", "liike", "building", "rakennus",
                     "kiinteistö", "property", "city"]),
    "pilvenpiirtaja": ("prop.building.skyscraper_v1", "far", None,
                       ["pilvenpiirtäjä", "torni", "skyscraper", "tower", "office", "building", "rakennus",
                        "kiinteistö", "property", "skyline", "city"]),
    "polttoainemittari": ("prop.street.fuel_pump_v1", "mid", 1.9,
                          ["polttoainemittari", "bensapumppu", "fuel_pump", "petrol", "huoltoasema",
                           "gas_station", "city"]),
    "torikoju": ("prop.shop.market_stall_v1", "mid", 2.4,
                 ["torikoju", "koju", "market_stall", "stall", "tori", "market", "kauppa", "shop", "city"]),
}


def _num(value):
    value = round(value, 3)
    return int(value) if value == int(value) else value


def _scaled(content, s):
    content = json.loads(json.dumps(content))
    if "rect" in content:
        content["rect"] = {k: _num(v * s) for k, v in content["rect"].items()}
    if "points" in content:
        content["points"] = [{k: _num(v * s) for k, v in p.items()} for p in content["points"]]
        content["width"] = _num(min(max(content["width"] * s, 0.1), MAX_LINE))
    return content


def manifest(key):
    prop = CITY_PROPS[key]
    asset_id, depth, metres, tags = CATALOG[key]
    scale = BUILDING_SCALE if metres is None else metres * PX_PER_METRE / prop["height"]
    width, height = _num(prop["width"] * scale), _num(prop["height"] * scale)
    layers = []
    for layer in prop["layers"]:
        # Library art has no clip rectangles; every clipped prop layer is
        # hidden below the clip by the body drawn after it.
        layers.append(dict(id=layer["id"][len(key) + 1:], content=_scaled(layer["content"], scale)))
    anchors = [dict(type="ground", id="floor", at=dict(x=0, y=height), width=width)]
    if key == "puistonpenkki":
        anchors.append(dict(type="seat", id="seat", at=dict(x=_num(width / 2), y=_num(130 * scale))))
    if key == "pankkiautomaatti":
        anchors.append(dict(type="screen", id="screen",
                            rect=dict(x=_num(70 * scale), y=_num(118 * scale),
                                      width=_num(160 * scale), height=_num(110 * scale))))
    return dict(
        schema_version=1, id=asset_id, kind="prop", name=prop["name"],
        bounds=dict(x=0, y=0, width=width, height=height), depth=depth,
        tags=list(dict.fromkeys(tags)),
        license=dict(license="MIT", author="Koeta Studio", origin="original", redistribution="allowed"),
        geometry=dict(type="art", layers=layers), anchors=anchors,
    )


def generate(directory=DEFAULT):
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    for key in CATALOG:
        data = manifest(key)
        text = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
        (directory / f"{data['id']}.json").write_text(text, encoding="utf-8")
    return len(CATALOG)


if __name__ == "__main__":
    print(json.dumps(dict(props=generate())))
