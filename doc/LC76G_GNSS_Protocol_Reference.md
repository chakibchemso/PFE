# LC26G(AB) & LC76G & LC86G Series — GNSS Protocol Specification

> **Document:** Quectel GNSS Protocol Specification  
> **Version:** 1.1  
> **Date:** 2022-12-20  
> **Status:** Released  
> **Focus:** LC76G Series (AB, PA, PB variants)

---

## 1. Supported Modules & Frequency Bands

| Module | Variant | Frequency Band |
|--------|---------|---------------|
| LC26G | LC26G (AB) | GPS L1 C/A + GLONASS L1 + Galileo E1 + BDS B1I + QZSS L1 C/A |
| **LC76G** | **LC76G (AB)** | **GPS L1 C/A + GLONASS L1 + Galileo E1 + BDS B1I + QZSS L1 C/A** |
| **LC76G** | **LC76G (PA)** | **GPS L1 C/A + GLONASS L1 + Galileo E1 + BDS B1I + QZSS L1 C/A** |
| **LC76G** | **LC76G (PB)** | **GPS L1 C/A + GLONASS L1 + Galileo E1 + BDS B1I + QZSS L1 C/A** |
| LC86G | LC86G (AA) | GPS L1 C/A + Galileo E1 + BDS B1I |
| LC86G | LC86G (AB) | GPS L1 C/A + GLONASS L1 + Galileo E1 |
| LC86G | LC86G (LA) | GPS L1 C/A + GLONASS L1 + Galileo E1 + BDS B1I + QZSS L1 C/A |

### Supported Protocols

| Protocol | Type | Direction |
|----------|------|-----------|
| NMEA 0183 V4.10 | ASCII, standard | Output |
| PAIR (MTK Proprietary) | ASCII, proprietary | Input/Output |
| PQTM (Quectel Proprietary) | ASCII, proprietary | Input/Output |
| RTCM 10403.3 | Binary, proprietary | Output |

---

## 2. NMEA Message Structure

```
$<Address>{,<Data>}*<Checksum><CR><LF>
```

| Field | Description |
|-------|-------------|
| `$` | Start of sentence (0x24) |
| `<Address>` | TalkerID (2 chars) + SentenceFormatter (3 chars). For proprietary: `P` + 3-char mnemonic |
| `<Data>` | Comma-delimited data fields, variable length |
| `*` | Checksum delimiter |
| `<Checksum>` | 8-bit XOR of all characters between `$` and `*` (exclusive), as two ASCII hex chars |
| `<CR><LF>` | End of sentence (0x0D 0x0A) |

### Checksum Calculation (C)

```c
unsigned char Ql_Check_XOR(const unsigned char *pData, unsigned int Length)
{
    unsigned char result = 0;
    unsigned int i = 0;
    if ((NULL == pData) || (Length < 1)) {
        return 0;
    }
    for (i = 0; i < Length; i++) {
        result ^= *(pData + i);
    }
    return result;
}
```

### NMEA Talker IDs (V4.10)

| Constellation | Talker ID |
|---------------|-----------|
| GPS | GP |
| GLONASS | GL |
| Galileo | GA |
| BDS | GB |
| QZSS | GP |
| Multi-constellation | GN |

---

## 3. Standard NMEA Messages

### 3.1 RMC — Recommended Minimum Specific GNSS Data

**Type:** Output

**Synopsis:**
```
$<TalkerID>RMC,<UTC>,<Status>,<Lat>,<N/S>,<Lon>,<E/W>,<SOG>,<COG>,<Date>,<MagVar>,<MagVarDir>,<ModeInd>,<NavStatus>*<Checksum><CR><LF>
```

| Field | Format | Unit | Example | Description |
|-------|--------|------|---------|-------------|
| `<TalkerID>` | 2 char | — | GN | Talker identifier |
| `<UTC>` | hhmmss.sss | — | 040143.000 | Fix UTC (hh: 00–23, mm: 00–59, ss: 00–59) |
| `<Status>` | char | — | A | A=Valid, V=Warning |
| `<Lat>` | ddmm.mmmmmm | — | 3149.334166 | Latitude (dd: 00–90, mm: 00–59) |
| `<N/S>` | char | — | N | N=North, S=South |
| `<Lon>` | dddmm.mmmmmm | — | 11706.941670 | Longitude (ddd: 000–180) |
| `<E/W>` | char | — | E | E=East, W=West |
| `<SOG>` | numeric | knots | 0.01 | Speed over ground |
| `<COG>` | numeric | degrees | 0.00 | Course over ground (max 359.99) |
| `<Date>` | ddmmyy | — | 010522 | Day, Month, Year |
| `<MagVar>` | — | — | — | Not supported |
| `<MagVarDir>` | — | — | — | Not supported |
| `<ModeInd>` | char | — | D | A=Autonomous, D=Differential, E=Dead reckoning, F=Float RTK, M=Manual, N=No fix, R=RTK |
| `<NavStatus>` | char | — | V | Always V (NMEA V4.10+) |

**LC76G (AB) Example:**
```
$GNRMC,040143.000,A,3149.334166,N,11706.941670,E,0.01,0.00,010522,,,D,V*0E
```

---

### 3.2 GGA — Global Positioning System Fix Data

**Type:** Output

**Synopsis:**
```
$<TalkerID>GGA,<UTC>,<Lat>,<N/S>,<Lon>,<E/W>,<Quality>,<NumSatUsed>,<HDOP>,<Alt>,M,<Sep>,M,<DiffAge>,<DiffStation>*<Checksum><CR><LF>
```

| Field | Format | Unit | Example | Description |
|-------|--------|------|---------|-------------|
| `<TalkerID>` | 2 char | — | GN | Talker identifier |
| `<UTC>` | hhmmss.sss | — | 040143.000 | Fix UTC |
| `<Lat>` | ddmm.mmmmmm | — | 3149.334166 | Latitude |
| `<N/S>` | char | — | N | N=North, S=South |
| `<Lon>` | dddmm.mmmmmm | — | 11706.941670 | Longitude |
| `<E/W>` | char | — | E | E=East, W=West |
| `<Quality>` | 1 digit | — | 2 | 0=Invalid, 1=GPS SPS, 2=DGPS/SBAS, 3=PPS, 4=RTK fixed, 5=Float RTK, 6=Dead reckoning |
| `<NumSatUsed>` | 2 digits | — | 36 | Satellites in use (may exceed 12 in multi-constellation) |
| `<HDOP>` | numeric | — | 0.48 | Horizontal dilution of precision |
| `<Alt>` | numeric | meters | 61.496 | Altitude above MSL |
| `<Sep>` | numeric | meters | -0.335 | Geoid separation |
| `<DiffAge>` | — | — | — | Not supported |
| `<DiffStation>` | — | — | — | Not supported |

**LC76G (AB) Example:**
```
$GNGGA,040143.000,3149.334166,N,11706.941670,E,2,36,0.48,61.496,M,-0.335,M,,*58
```

> **Note:** GGA messages are GPS-specific per NMEA 0183, but with multi-constellation the content is generated from the multi-constellation solution.

---

### 3.3 GSV — GNSS Satellites in View

**Type:** Output

**Synopsis:**
```
$<TalkerID>GSV,<TotalNumSen>,<SenNum>,<TotalNumSat>{,<SatID>,<SatElev>,<SatAz>,<SatCN0>},<SignalID>*<Checksum><CR><LF>
```

| Field | Format | Unit | Description |
|-------|--------|------|-------------|
| `<TalkerID>` | 2 char | — | GP/GL/GA/GB (GN cannot be used for GSV) |
| `<TotalNumSen>` | numeric | — | Total sentences (1–9) |
| `<SenNum>` | numeric | — | Current sentence number (1–TotalNumSen) |
| `<TotalNumSat>` | numeric | — | Total satellites in view |
| `<SatID>` | numeric | — | Satellite ID (see Appendix B) |
| `<SatElev>` | numeric | degrees | Elevation (00–90) |
| `<SatAz>` | numeric | degrees | Azimuth (000–359) |
| `<SatCN0>` | numeric | dB-Hz | C/N0 (00–99), null when not tracking |
| `<SignalID>` | numeric | — | Signal ID (NMEA V4.10+) |

Repeat block: 1–4 satellites per sentence.

**LC76G (AB) Example:**
```
$GPGSV,3,1,12,195,72,076,42,01,69,158,45,194,66,111,29,21,61,060,44,1*6D
$GPGSV,3,2,12,07,61,233,42,30,52,284,44,199,51,162,37,08,39,045,42,1*59
$GPGSV,3,3,12,14,29,312,29,196,20,148,36,17,18,258,36,27,07,061,36,1*53
$GLGSV,2,1,05,79,80,068,47,82,62,248,44,81,56,014,38,78,31,137,24,1*7F
$GLGSV,2,2,05,88,07,034,29,1*46
$GAGSV,2,1,06,26,80,095,42,01,69,353,13,21,49,106,26,33,42,207,41,7*72
$GAGSV,2,2,06,13,28,040,34,31,19,313,34,7*72
$GBGSV,4,1,16,46,81,194,38,07,68,349,31,40,61,016,40,30,60,259,43,1*71
$GBGSV,4,2,16,10,59,321,,03,51,192,36,36,41,314,38,02,37,229,32,1*71
$GBGSV,4,3,16,09,31,219,26,08,27,175,31,37,25,146,29,06,23,202,29,1*78
$GBGSV,4,4,16,16,20,199,31,13,17,186,26,39,12,192,29,28,09,048,30,1*7C
```

> **Note:** GN talker cannot be used for GSV. Use separate sentences per constellation with the corresponding TalkerID.

---

### 3.4 GSA — GNSS DOP and Active Satellites

**Type:** Output

**Synopsis:**
```
$<TalkerID>GSA,<Mode>,<FixMode>{,<SatID>},<PDOP>,<HDOP>,<VDOP><SystemID>*<Checksum><CR><LF>
```

| Field | Format | Description |
|-------|--------|-------------|
| `<Mode>` | char | M=Manual 2D/3D, A=Automatic 2D/3D |
| `<FixMode>` | numeric | 1=No fix, 2=2D, 3=3D |
| `<SatID>` ×12 | numeric | Satellite IDs used in solution |
| `<PDOP>` | numeric | Position DOP (max 99.00) |
| `<HDOP>` | numeric | Horizontal DOP (max 99.00) |
| `<VDOP>` | numeric | Vertical DOP (max 99.00) |
| `<SystemID>` | numeric | GNSS system ID (NMEA V4.10+) |

**LC76G (AB) Example:**
```
$GNGSA,A,3,195,01,194,21,07,30,199,08,14,17,27,,0.71,0.48,0.52,1*34
$GNGSA,A,3,79,82,81,78,88,,,,,,,,0.71,0.48,0.52,2*0D
$GNGSA,A,3,26,21,33,13,31,,,,,,,,0.71,0.48,0.52,3*09
$GNGSA,A,3,46,07,40,30,03,36,02,09,08,37,06,16,0.71,0.48,0.52,4*0B
```

> **Note:** If < 12 satellites used, remaining fields are empty. If > 12 used, only first 12 IDs are output.

---

### 3.5 VTG — Course Over Ground & Ground Speed

**Type:** Output

**Synopsis:**
```
$<TalkerID>VTG,<COGT>,T,<COGM>,M,<SOGN>,N,<SOGK>,K,<ModeInd>*<Checksum><CR><LF>
```

| Field | Format | Unit | Description |
|-------|--------|------|-------------|
| `<COGT>` | numeric | degrees | Course over ground (true north) |
| `<COGM>` | numeric | degrees | Course over ground (magnetic) — Not supported |
| `<SOGN>` | numeric | knots | Speed over ground (knots) |
| `<SOGK>` | numeric | km/h | Speed over ground (km/h) |
| `<ModeInd>` | char | — | A=Autonomous, D=Differential, E=Dead reckoning, F=Float RTK, M=Manual, N=No fix, R=RTK |

**LC76G (AB) Example:**
```
$GNVTG,0.00,T,,M,0.01,N,0.02,K,D*25
```

---

### 3.6 GLL — Geographic Position – Latitude/Longitude

**Type:** Output

**Synopsis:**
```
$<TalkerID>GLL,<Lat>,<N/S>,<Lon>,<E/W>,<UTC>,<Status>,<ModeInd>*<Checksum><CR><LF>
```

| Field | Format | Unit | Example | Description |
|-------|--------|------|---------|-------------|
| `<Lat>` | ddmm.mmmmmm | — | 3149.334166 | Latitude |
| `<N/S>` | char | — | N | N=North, S=South |
| `<Lon>` | dddmm.mmmmmm | — | 11706.941670 | Longitude |
| `<E/W>` | char | — | E | E=East, W=West |
| `<UTC>` | hhmmss.sss | — | 040143.000 | Fix UTC |
| `<Status>` | char | — | A | A=Valid, V=Invalid |
| `<ModeInd>` | char | — | D | Mode indicator (A/D/E/F/M/N/R) |

**LC76G (AB) Example:**
```
$GNGLL,3149.334166,N,11706.941670,E,040143.000,A,D*46
```

---

### 3.7 ZDA — Time and Date

**Type:** Output

**Synopsis:**
```
$<TalkerID>ZDA,<UTC>,<Day>,<Month>,<Year>,<LocalHour>,<LocalMin>*<Checksum><CR><LF>
```

| Field | Format | Description |
|-------|--------|-------------|
| `<UTC>` | hhmmss.sss | Fix UTC |
| `<Day>` | numeric | Day (01–31) |
| `<Month>` | numeric | Month (01–12) |
| `<Year>` | numeric | Year |
| `<LocalHour>` | numeric | Local zone hours — **Not supported** |
| `<LocalMin>` | numeric | Local zone minutes — **Not supported** |

**LC76G (AB) Example:**
```
$GNZDA,055054.000,19,09,2022,,*4A
```

> **Note:** LC76G modules do not support local time output due to firmware limitation.

---

### 3.8 GNS — GNSS Fix Data

**Type:** Output

**Synopsis:**
```
$<TalkerID>GNS,<UTC>,<Lat>,<N/S>,<Lon>,<E/W>,<ModeInd>,<NumSatUsed>,<HDOP>,<Alt>,M,<Sep>,M,<DiffAge>,<DiffStation>,<NavStatus>*<Checksum><CR><LF>
```

| Field | Format | Description |
|-------|--------|-------------|
| `<ModeInd>` | variable length | **Multi-char:** 1st=GPS, 2nd=GLONASS, 3rd=Galileo, 4th=BDS, 5th=QZSS, 6th=NavIC |
| `<NumSatUsed>` | numeric | Satellites in use (0–99) |
| `<HDOP>` | numeric | Horizontal DOP (max 99.00) |
| `<Alt>` | numeric (meters) | Antenna altitude above MSL |
| `<Sep>` | numeric (meters) | Geoid separation |
| `<NavStatus>` | char | Always "V" |

**LC76G (AB) Example:**
```
$GNGNS,053106.000,3149.334190,N,11706.948654,E,DANN,16,0.63,51.287,M,-0.335,M,,,V*05
```

> **Note:** LC76G (AB) with LC76GABNR02A01, LC26G (AB) with LC26GABNR02A01 — these and earlier versions do NOT support GNS output.

---

### 3.9 GST — GNSS Pseudorange Error Statistics

**Type:** Output (supports RAIM)

**Synopsis:**
```
$<TalkerID>GST,<UTC>,<RMS_D>,<MajorD>,<MinorD>,<Orient>,<LatD>,<LonD>,<AltD>*<Checksum><CR><LF>
```

| Field | Format | Unit | Description |
|-------|--------|------|-------------|
| `<UTC>` | hhmmss.sss | — | UTC of associated GGA/GNS fix |
| `<RMS_D>` | numeric | meters | RMS of range residuals standard deviation |
| `<MajorD>` | numeric | meters | Semi-major axis of error ellipse |
| `<MinorD>` | numeric | meters | Semi-minor axis of error ellipse |
| `<Orient>` | numeric | degrees | Orientation of semi-major axis |
| `<LatD>` | numeric | meters | Latitude error standard deviation |
| `<LonD>` | numeric | meters | Longitude error standard deviation |
| `<AltD>` | numeric | meters | Altitude error standard deviation |

**LC76G (AB) Example:**
```
$GNGST,123624.000,6.3,2.5,2.4,88.4,2.4,2.5,9.2*43
```

---

### 3.10 GRS — GNSS Range Residuals

**Type:** Output (supports RAIM)

**Synopsis:**
```
$<TalkerID>GRS,<UTC>,<Mode>{,<Resi>},<SystemID>,<SignalID>*<Checksum><CR><LF>
```

| Field | Format | Unit | Description |
|-------|--------|------|-------------|
| `<Mode>` | numeric | — | 0=Residuals used in GGA/GNS, 1=Recomputed after fix |
| `<Resi>` ×12 | numeric | meters | Range residuals (-999 to 999) |
| `<SystemID>` | numeric | — | GNSS system ID (V4.10+) |
| `<SignalID>` | numeric | — | GNSS signal ID (V4.10+) |

**LC76G (AB) Example:**
```
$GNGRS,125524.000,1,-0.4,-0.7,0.5,-4.6,0.2,1.1,-2.2,-0.6,-1.1,9.2,-2.1,3.1,1,1*42
```

---

### 3.11 RLM — Return Link Message

**Type:** Output (Galileo SAR)

**Synopsis:**
```
$<TalkerID>RLM,<BeaconID>,<UTC>,<Meg_Code>,<Para>*<Checksum><CR><LF>
```

| Field | Format | Description |
|-------|--------|-------------|
| `<BeaconID>` | 15 hex chars (60 bits) | Beacon ID |
| `<UTC>` | hhmmss.sss | Fix UTC |
| `<Meg_Code>` | hex char (4 bits) | 0=Reserved, 1=Acknowledgement, 2=Command, 3=Message, F=Test |
| `<Para>` | 4 or 24 hex chars | Data parameters (16 or 96 bits) |

**LC76G (AB) Example:**
```
$GARLM,9A22BE29630F010,125713.000,F,5402*3B
```

> **Note:** RLM must be enabled via `$PAIR154`. LC76GABNR02A01 and earlier versions do not support RLM output.

---

## 4. PAIR Messages (MTK Proprietary)

PAIR messages are proprietary NMEA messages. All PAIR commands return an acknowledgement via `$PAIR001`. For LC86G: resend commands until `$PAIR001` is returned.

### 4.1 Acknowledgement Format — PAIR001

```
$PAIR001,<CommandID>,<Result>*<Checksum><CR><LF>
```

| `<Result>` | Meaning |
|-----------|---------|
| 0 | Command successfully sent |
| 1 | Command being processed — wait for result |
| 2 | Command sending failed |
| 3 | `<CommandID>` not supported |
| 4 | Parameter error (out of range / missing / checksum error) |
| 5 | MNL service busy — try again soon |

---

### 4.2 System Power & Start Commands (002–007, 010)

#### PAIR002 — GNSS Subsystem Power On

**Type:** Command

```
$PAIR002*38
```

Powers on DSP, RF, PE, and clock. Returns `$PAIR001`.

> **LC76G/LC26G note:** After `$PAIR003`, use `$PAIR002` to power on again. LC86G does NOT support using `$PAIR002` after `$PAIR003`.

---

#### PAIR003 — GNSS Subsystem Power Off

**Type:** Command

```
$PAIR003*39
```

- **LC76G/LC26G:** CPU enters **Standby mode** (can still receive commands)
- **LC86G:** CPU enters **Sleep mode** (cannot receive commands unless `$PAIR382,1` was sent first)

> LC86G: Send `$PAIR382,1*2E` before `$PAIR003` to keep receiving commands in Sleep mode.

---

#### PAIR004 — Hot Start

**Type:** Command

```
$PAIR004*3E
```

Uses all NVRAM data (ephemeris still valid, < 2h power-down with RTC alive). Fastest startup.

---

#### PAIR005 — Warm Start

**Type:** Command

```
$PAIR005*3F
```

Remembers rough time, position, almanac. Needs ephemeris download.

---

#### PAIR006 — Cold Start

**Type:** Command

```
$PAIR006*3C
```

No location/time/almanac/ephemeris stored. Full search needed.

---

#### PAIR007 — Full Cold Start (Factory Reset)

**Type:** Command

```
$PAIR007*3D
```

Cold start + clears user/system configs to factory settings.

---

#### PAIR010 — Aiding Data Expiration (Output)

**Type:** Output (automatic on power-up)

```
$PAIR010,<Type>,<GNSS_System>,<WN>,<TOW>*<Checksum>
```

| Field | Description |
|-------|-------------|
| `<Type>` | 0=EPO, 1=Time, 2=Location |
| `<GNSS_System>` | 0=GPS, 1=GLONASS, 2=Galileo, 3=BDS, 4=QZSS |
| `<WN>` | Week number (with roll-over) |
| `<TOW>` | Time of week (seconds) |

> Do NOT send `$PAIR010` manually — it's auto-output.

---

### 4.3 Common Configuration (050–087)

#### PAIR050 — Set Fix Rate

**Type:** Set

```
$PAIR050,<Time>*<Checksum>
```

| Param | Range | Default |
|-------|-------|---------|
| `<Time>` | 100–1000 ms | 1000 (1 Hz) |

> **LC76G (PA)/(PB): NOT supported.** Fix rate remains at 1 Hz.  
> If rate > 1 Hz: only RMC, GGA, GNS output at set rate; VTG, GLL, ZDA, GRS, GST are suppressed; GSA, GSV output at 1 Hz.

**Example:**
```
$PAIR050,1000*12
$PAIR001,050,0*3E
```

---

#### PAIR051 — Get Fix Rate

**Type:** Get

```
$PAIR051*3E
→ $PAIR051,<Time>*<Checksum>
```

---

#### PAIR058 — Set Minimum SNR

**Type:** Set

```
$PAIR058,<MIN_SNR>*<Checksum>
```

| Param | Range | Default |
|-------|-------|---------|
| `<MIN_SNR>` | 9–37 dB | 9 |

Satellites with SNR below threshold are excluded from positioning.

**Example:**
```
$PAIR058,15*1F
```

---

#### PAIR059 — Get Minimum SNR

**Type:** Get

```
$PAIR059*36
→ $PAIR059,<MIN_SNR>*<Checksum>
```

---

#### PAIR062 — Set NMEA Output Rate

**Type:** Set

```
$PAIR062,<Type>,<OutputRate>*<Checksum>
```

| `<Type>` | Sentence | `<OutputRate>` |
|----------|----------|---------------|
| -1 | Reset ALL | — |
| 0 | GGA | 0=Disabled, N=every N fixes (1–20, default 1) |
| 1 | GLL | |
| 2 | GSA | |
| 3 | GSV | |
| 4 | RMC | |
| 5 | VTG | |
| 6 | ZDA | |
| 7 | GRS | |
| 8 | GST | |
| 9 | GNS | |

> LC76GABNR02A01S and earlier: GNS (type 9) not supported.

**Example:**
```
$PAIR062,0,3*3D    # Output GGA every 3 fixes
```

---

#### PAIR063 — Get NMEA Output Rate

**Type:** Get

```
$PAIR063,<Type>*<Checksum>
→ $PAIR063,<Type>,<OutputRate>*<Checksum>
```

---

#### PAIR066 — Set GNSS Search Mode

**Type:** Set (causes reboot)

```
$PAIR066,<GPS>,<GLONASS>,<Galileo>,<BDS>,<QZSS>,<Reserved>*<Checksum>
```

Each constellation: 0=Disable, 1=Search. `<Reserved>` always 0. QZSS is always enabled by default.

**LC76G supported modes:**
- GPS only
- GPS + QZSS
- GPS + GLONASS
- GPS + GLONASS + QZSS
- GPS + Galileo
- GPS + Galileo + QZSS
- GPS + BDS
- GPS + BDS + QZSS
- GPS + GLONASS + Galileo + BDS
- GPS + GLONASS + Galileo + BDS + QZSS

**Example (GPS+GLONASS+Galileo+BDS):**
```
$PAIR066,1,1,1,1,0,0*3A
```

---

#### PAIR067 — Get GNSS Search Mode

**Type:** Get

```
$PAIR067*3B
→ $PAIR067,<GPS>,<GLONASS>,<Galileo>,<BDS>,<QZSS>,<Reserved>*<Checksum>
```

---

#### PAIR070 — Set Static Navigation Threshold

**Type:** Set

```
$PAIR070,<SpeedThreshold>*<Checksum>
```

| Param | Range | Default |
|-------|-------|---------|
| `<SpeedThreshold>` | 0–20 dm/s | 0 (disabled) |

If actual speed < threshold: position held constant, speed output as 0.

---

#### PAIR071 — Get Static Navigation Threshold

**Type:** Get

```
$PAIR071*3C
→ $PAIR071,<SpeedThreshold>*<Checksum>   (unit: m/s)
```

---

#### PAIR072 — Set Elevation Mask

**Type:** Set

```
$PAIR072,<Degree>*<Checksum>
```

| Param | Range | Default |
|-------|-------|---------|
| `<Degree>` | -90 to 90 | 5 |

Satellites below mask cannot be used for positioning.

---

#### PAIR073 — Get Elevation Mask

**Type:** Get

```
$PAIR073*3E
→ $PAIR073,<Degree>*<Checksum>
```

---

#### PAIR074 — Set AIC (Active Interference Cancellation)

**Type:** Set

```
$PAIR074,<Enabled>*<Checksum>
```

`<Enabled>`: 0=Disable, 1=Enable

---

#### PAIR075 — Get AIC Status

**Type:** Get

```
$PAIR075*38
→ $PAIR075,<Status>*<Checksum>
```

---

#### PAIR080 — Set Navigation Mode

**Type:** Set

```
$PAIR080,<NavMode>*<Checksum>
```

| Value | Mode |
|-------|------|
| 0 | Normal (general purpose) |
| 1 | Fitness (running/walking, low-speed < 5 m/s emphasized) |
| 2–4 | Reserved |
| 5 | Drone (hovering, cruising — dynamic range + vertical accel) |
| 6 | Reserved |
| 7 | Swimming (smooth trajectory, accurate distance) |

> When NavMode=5: Lat/Lon decimal places in RMC, GGA, GLL, GNS increase from 6 to 7 (except LC76GABNR02A01S and earlier).

**Example:**
```
$PAIR080,1*2F    # Fitness mode
```

---

#### PAIR081 — Get Navigation Mode

**Type:** Get

```
$PAIR081*33
→ $PAIR081,<NavMode>*<Checksum>
```

---

#### PAIR086 — Set Debug Log Output

**Type:** Set

```
$PAIR086,<Status>*<Checksum>
```

| Value | Mode |
|-------|------|
| 0 | Disable |
| 1 | Enable (full debug log) |
| 2 | Enable (lite debug log) |

---

#### PAIR087 — Get Debug Log Output

**Type:** Get

```
$PAIR087*35
→ $PAIR087,<Status>*<Checksum>
```

---

### 4.4 Advanced Features (154–158)

#### PAIR154 — Set RLM Output Enable

**Type:** Set

```
$PAIR154,<Enable>*<Checksum>
```

`<Enable>`: 0=Disable, 1=Enable (output at 1 Hz)

> LC76GABNR02A01S and earlier: NOT supported.

---

#### PAIR155 — Get RLM Output Status

**Type:** Get

```
$PAIR155*3B
→ $PAIR155,<Enable>*<Checksum>
```

---

#### PAIR158 — Set BDS B1C Band Tracking

**Type:** Set

```
$PAIR158,<Enable>*<Checksum>
```

`<Enable>`: 0=Disable, 1=Enable BDS B1C tracking.

> LC26G (AB) does NOT support this. LC76G: supported from LC76GPANR02A02S / LC76GPBNR02A02S onward when constellation = GPS+BDS(+QZSS).

---

### 4.5 System Lock & Jamming (382, 391)

#### PAIR382 — Lock System Sleep

**Type:** Set

```
$PAIR382,<Enabled>*<Checksum>
```

Prevents CPU from auto-entering Sleep mode. **Not saved** — must be resent after each reboot.

`<Enabled>`: 0=Disable lock, 1=Enable lock.

> **LC86G critical:** Send `$PAIR382,1*2E` before `$PAIR003*39` to receive commands in Sleep mode. Always send `$PAIR382,1*2E` before other commands on LC86G.

---

#### PAIR391 — Jamming Detection

**Type:** Set

```
$PAIR391,<CmdType>*<Checksum>
```

`<CmdType>`: 0=Disable, 1=Enable. Returns jamming status via `$PAIRSPF`:

| PAIRSPF Status | Meaning |
|---------------|---------|
| `$PAIRSPF,0` | Unknown |
| `$PAIRSPF,1` | Good (no jamming) |
| `$PAIRSPF,2` | Warning |
| `$PAIRSPF,3` | Critical |

Jamming detection starts immediately. If continuous jamming, status progresses 1→2→3.

---

### 4.6 DGPS & SBAS (400–411)

#### PAIR400 — Set DGPS Data Source

**Type:** Set

```
$PAIR400,<Mode>*<Checksum>
```

| Value | Source |
|-------|--------|
| 0 | No DGPS |
| 1 | RTCM |
| 2 | SBAS (WAAS/EGNOS/GAGAN/MSAS) |

---

#### PAIR401 — Get DGPS Data Source

**Type:** Get

```
$PAIR401*3F
→ $PAIR401,<Mode>*<Checksum>
```

---

#### PAIR410 — Enable/Disable SBAS

**Type:** Set

```
$PAIR410,<Enabled>*<Checksum>
```

`<Enabled>`: 0=Disable, 1=Enable.

> SBAS is NOT supported in Fitness or Swimming navigation modes (see PAIR080).

---

#### PAIR411 — Get SBAS Status

**Type:** Get

```
$PAIR411*3E
→ $PAIR411,<Enabled>*<Checksum>
```

---

### 4.7 RTCM Output (432–437)

#### PAIR432 — Set RTCM Output Mode

**Type:** Set

```
$PAIR432,<Mode>*<Checksum>
```

| Value | Mode |
|-------|------|
| -1 | Disable RTCM output |
| 0 | RTCM3 with MSM4 |
| 1 | RTCM3 with MSM7 |

---

#### PAIR433 — Get RTCM Output Mode

**Type:** Get

```
$PAIR433*3E
→ $PAIR433,<Mode>*<Checksum>
```

---

#### PAIR434 — Set RTCM Antenna Reference Point Output

**Type:** Set

```
$PAIR434,<Enable>*<Checksum>
```

Enables/disables RTCM message type 1005 (Stationary RTK Reference Station ARP).

---

#### PAIR435 — Get RTCM Antenna Reference Point Setting

**Type:** Get

```
$PAIR435*38
→ $PAIR435,<Enable>*<Checksum>
```

---

#### PAIR436 — Set RTCM Ephemeris Output

**Type:** Set

```
$PAIR436,<Enable>*<Checksum>
```

Enables/disables ephemeris messages (1019, 1020, 1042, 1044, 1046).

---

#### PAIR437 — Get RTCM Ephemeris Setting

**Type:** Get

```
$PAIR437*3A
→ $PAIR437,<Enable>*<Checksum>
```

---

### 4.8 EASY, NVRAM & Low Power (490–650)

#### PAIR490 — Enable/Disable EASY (Embedded Assist System)

**Type:** Set

```
$PAIR490,<Enabled>*<Checksum>
```

`<Enabled>`: 0=Disable, 1=Enable.

---

#### PAIR491 — Get EASY Status

**Type:** Get

```
$PAIR491*36
→ $PAIR491,<Enabled>,<Status>*<Checksum>
```

| `<Status>` | Meaning |
|-----------|---------|
| 0 | Not finished |
| 1 | 1-day extension finished |
| 2 | 2-day extension finished |
| 3 | 3-day extension finished |

> If `<Enabled>=0`, `<Status>` is not displayed.

---

#### PAIR511 — Save Navigation Data to Flash

**Type:** Command

```
$PAIR511*3F
```

Saves current navigation data from RTC RAM to flash.

> **Important:** If RTC loses power after module power-off, send this after each parameter change. For fix rates > 1 Hz: send `$PAIR382,1*2E` + `$PAIR003*39` first, then `$PAIR511*3F`, then `$PAIR002*38` to re-power.

---

#### PAIR513 — Save Configuration to Flash

**Type:** Command

```
$PAIR513*3D
```

Saves current configurations from RTC RAM to flash. Same power-off requirements as PAIR511 for multi-Hz scenarios.

---

#### PAIR650 — Enter RTC Backup Mode

**Type:** Set

```
$PAIR650,<Second>*<Checksum>
```

| Param | Range | Description |
|-------|-------|-------------|
| `<Second>` | 0 or 10–62208000 | Duration in Backup mode. 0 = no timer. Max = 2 years. |

CPU enters Backup mode (clock only). **Cannot receive any commands** in this mode.

---

### 4.9 Low Power Modes (680–733)

#### PAIR680 — Enable GLP (GPS Low Power)

**Type:** Set

```
$PAIR680,<Enabled>*<Checksum>
```

**Requirements to enter GLP mode:**
1. Fix rate = 1 Hz
2. Constellation = GPS only
3. Navigation mode = Fitness mode (PAIR080,1)

> When GLP enabled: SBAS, ALP, FLP, and periodic power saving are auto-disabled.

---

#### PAIR681 — Get GLP Status

**Type:** Get

```
$PAIR681*35
→ $PAIR681,<Enabled>*<Checksum>
```

---

#### PAIR690 — Set Periodic Power Saving Mode

**Type:** Set

```
$PAIR690,<Mode>,<FirstRun>,<FirstSleep>,<SecondRun>,<SecondSleep>*<Checksum>
```

| Param | Range | Description |
|-------|-------|-------------|
| `<Mode>` | — | 0=Disabled, 1=Smart periodic, 2=Strict periodic |
| `<FirstRun>` | 3–518400 s | Run time after exiting sleep (with fix) |
| `<FirstSleep>` | 3–518400 s | Sleep time after getting/attempting fix |
| `<SecondRun>` | 0 or 3–518400 s | Run time when NO signal (0 only when SecondSleep=0) |
| `<SecondSleep>` | 0 or 3–518400 s | Sleep time when NO signal (0 only when SecondRun=0) |

**Example:**
```
$PAIR690,1,21,39,48,72*28
```

---

#### PAIR691 — Get Periodic Power Saving Mode

**Type:** Get

```
$PAIR691*34
→ $PAIR691,<Mode>,<FirstRun>,<FirstSleep>,<SecondRun>,<SecondSleep>*<Checksum>
```

---

#### PAIR730 — Enable FLP (Fitness Low Power)

**Type:** Set

```
$PAIR730,<Enabled>*<Checksum>
```

**Requirements to enter FLP mode:**
1. Fix rate = 1 Hz
2. Navigation mode = Fitness (except LC76GABNR02A01S and earlier)
3. Constellation: GPS+GLONASS+Galileo+BDS(+QZSS), GPS+GLONASS(+QZSS), or GPS+BDS(+QZSS)

> When FLP enabled: SBAS, periodic mode, GLP, ALP are auto-disabled.

---

#### PAIR731 — Get FLP Status

**Type:** Get

```
$PAIR731*3F
→ $PAIR731,<Enabled>*<Checksum>
```

---

#### PAIR732 — Enable ALP (Adaptive Low Power)

**Type:** Set

```
$PAIR732,<Enabled>*<Checksum>
```

**Requirements to enter ALP mode:**
1. Fix rate = 1 Hz
2. Navigation mode = Normal mode
3. Constellation: GPS+GLONASS+Galileo+BDS(+QZSS) or GPS+GLONASS(+QZSS)

> LC76GABNR02A01S and earlier: NOT supported.  
> When ALP enabled: SBAS, periodic mode, FLP, GLP are auto-disabled.

---

#### PAIR733 — Get ALP Status

**Type:** Get

```
$PAIR733*3D
→ $PAIR733,<Enabled>*<Checksum>
```

---

### 4.10 PPS & UART (752, 864–865)

#### PAIR752 — Set PPS Configuration

**Type:** Set

```
$PAIR752,<PPSType>,<PPSPulseWidth>*<Checksum>
```

| `<PPSType>` | Mode |
|------------|------|
| 0 | Disable |
| 1 | After first fix |
| 2 | 3D fix only |
| 3 | 2D/3D fix only |
| 4 | Always |

`<PPSPulseWidth>`: 1–999 ms (default: 100).

**Example:**
```
$PAIR752,2,100*39
```

---

#### PAIR864 — Set UART Baud Rate

**Type:** Set (requires reboot)

```
$PAIR864,<PortType>,<PortIndex>,<Baudrate>*<Checksum>
```

| Param | Value |
|-------|-------|
| `<PortType>` | 0 = UART |
| `<PortIndex>` | 0 = UART0 |
| `<Baudrate>` | 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600 |

> Default baud rate recommended. Below 115200 may cause message loss.

**Example:**
```
$PAIR864,0,0,115200*1B
```

---

#### PAIR865 — Get UART Baud Rate

**Type:** Get

```
$PAIR865,<PortType>,<PortIndex>*<Checksum>
→ $PAIR865,<Baudrate>*<Checksum>
```

---

### 4.11 Geofence (890–891)

#### PAIR890 — Set Geofence Configuration

**Type:** Set

```
$PAIR890,<FenceNum>,<ConfLvl>{,<Lat1>,<Lon1>,<Rad1>}*<Checksum>
```

| Param | Range | Description |
|-------|-------|-------------|
| `<FenceNum>` | 0–4 | Number of geofences. 0 = disable. Max 4 |
| `<ConfLvl>` | 0–3 | 0=No requirement, 1=1-Sigma (68%), 2=2-Sigma (95%), 3=3-Sigma (99.7%) |
| `<Lat>` | degrees | Geofence circle center latitude |
| `<Lon>` | degrees | Geofence circle center longitude |
| `<Rad>` | meters | Geofence circle radius |

When `<FenceNum>≠0`: binary data is also output in this format:

```
0x04 0x24 | 0xDD 0x07 | 0x0C 0x00 | <12-byte payload> | <checksum> | 0xAA 0x44
```

**Binary payload (12 bytes):**

| Offset | Length | Name | Description |
|--------|--------|------|-------------|
| 0 | 1 | Status | 0=Disabled, 1=Enabled |
| 1 | 1 | Fencenum | Number of fences |
| 2 | 1 | State | 0=Outside, 1=Unknown, 2=Inside |
| 3 | 4 | EachState[4] | Per-fence state (0/1/2) |
| 7 | 1 | Hour | UTC hour (0–23) |
| 8 | 1 | Min | UTC minute (0–59) |
| 9 | 1 | Sec | UTC second (0–59) |
| 10 | 2 | Msec | UTC millisecond (0–999) |

All multi-byte values in Little Endian.

**Example:**
```
$PAIR890,1,1,25.0567,121.5743,30*20
$PAIR001,890,0*3A
04 24 DD 07 0C 00 01 01 00 00 00 00 00 05 27 39 00 00 CD AA 44
```

---

#### PAIR891 — Get Geofence Configuration

**Type:** Get

```
$PAIR891*3A
→ $PAIR891,<FenceNum>,<ConfLvl>{,<Lat1>,<Lon1>,<Rad1>}*<Checksum>
```

---

### 4.12 LOCUS Data Logging (900–909)

> LC76GABR02A01S and earlier: NOT supported.

#### PAIR900 — Enable/Disable LOCUS

**Type:** Set

```
$PAIR900,<Enable>*<Checksum>
```

`<Enable>`: 0=Disable, 1=Enable.

Saved data: UTC, fix status, longitude, latitude, altitude, ground speed, heading, HDOP, satellites used.

> Same config cannot be re-set after first successful `$PAIR900`.

---

#### PAIR901 — Get LOCUS Status

**Type:** Get

```
$PAIR901*32
→ $PAIR901,<Enable>*<Checksum>
```

---

#### PAIR902 — Set LOCUS Save Mode

**Type:** Set

```
$PAIR902,<Mode>,<Check_3D_Fix>*<Checksum>
```

`<Mode>` is bitmask (hex):

| Bit | Mode |
|-----|------|
| 0 | Normal — record each fix |
| 1 | Time-triggered (see PAIR904) |
| 2 | Speed-triggered (see PAIR904) |
| 3 | Distance-triggered (see PAIR904) |
| 4 | Before entering sleep |
| 5 | User control (via PAIR907) |

`<Check_3D_Fix>`: 0=Don't check, 1=Only save 3D fixes.

> LOCUS must be disabled before sending this command.

**Example (time + speed mode, 3D fix check):**
```
$PAIR902,6,1*36
```

---

#### PAIR903 — Get LOCUS Save Mode

**Type:** Get

```
$PAIR903*30
→ $PAIR903,<Mode>,<Check_3D_Fix>*<Checksum>
```

---

#### PAIR904 — Set LOCUS Threshold

**Type:** Set

```
$PAIR904,<Mode>,<Threshold>*<Checksum>
```

| `<Mode>` | Type | `<Threshold>` Range | Unit |
|----------|------|---------------------|------|
| 0 | Time | 1–43200 | seconds |
| 1 | Speed | 1–100 | m/s |
| 2 | Distance | 1–50000 | meters |

> LOCUS must be disabled, and mode set via PAIR902 first.

---

#### PAIR905 — Get LOCUS Threshold

**Type:** Get

```
$PAIR905,<Mode>*<Checksum>
→ $PAIR905,<Threshold>*<Checksum>
```

---

#### PAIR906 — Clear LOCUS Data

**Type:** Command

```
$PAIR906,<Type>*<Checksum>
```

| `<Type>` | Action |
|----------|--------|
| 0 | Clear records + restore default settings |
| 1 | Clear records only |
| 2 | Clear user settings + restore defaults |

---

#### PAIR907 — Log Current Fix Now

**Type:** Command

```
$PAIR907*34
```

> Bit 5 must be set in PAIR902 `<Mode>` first.

---

#### PAIR908 — Get LOCUS Data

**Type:** Command

```
$PAIR908,<Type>*<Checksum>
```

`<Type>`: 0=NMEA format (LOGGA + LORMC), 1=PAIR hex format.

**NMEA format response:**
```
$PAIR908,1,<Record_Num>,<Record_Size>*<Checksum>
$LOGGA,...  (same fields as GGA)
$LORMC,...  (same fields as RMC)
...
$PAIR908,3*24
```

**PAIR hex format response:**
```
$PAIR908,2,<UTC>,<Fix_Type>,<Lat>,<Lon>,<Height>,<Speed>,<Heading>,<HDOP>,<SatNo>*<Checksum>
```

| Hex Field | Bytes | Description |
|-----------|-------|-------------|
| `<UTC>` | 4 | UTC second |
| `<Fix_Type>` | 1 | Quality indicator (same as GGA) |
| `<Lat>` | 4 | WGS84 latitude |
| `<Lon>` | 4 | WGS84 longitude |
| `<Height>` | 2 | Altitude above MSL |
| `<Speed>` | 2 | Ground speed (m/s) |
| `<Heading>` | 2 | Heading of motion |
| `<HDOP>` | 2 | Horizontal DOP |
| `<SatNo>` | 2 | Satellites used |

> Must enable LOCUS via `$PAIR900,1*2E` first.

---

#### PAIR909 — Get LOCUS Record Count

**Type:** Command

```
$PAIR909*3A
→ $PAIR909,<Record_Num>*<Checksum>
```

---

### 4.13 Jamming Status Output — PAIRSPF

**Type:** Output (when jamming detection enabled via PAIR391)

```
$PAIRSPF,<Status>*<Checksum>
```

| Status | Meaning |
|--------|---------|
| 0 | Unknown |
| 1 | Good — no jamming |
| 2 | Warning |
| 3 | Critical |

---

## 5. PQTM Messages (Quectel Proprietary)

### PQTMCFGMSGRATE — Configure Message Output Rate

**Type:** Set/Get

```
# Set:
$PQTMCFGMSGRATE,W,<MsgName>,<Rate>[,<MsgVer>]*<Checksum>

# Get:
$PQTMCFGMSGRATE,R,<MsgName>[,<MsgVer>]*<Checksum>
```

| Param | Description |
|-------|-------------|
| `<MsgName>` | Message name (currently only `$PQTMEPE` supported) |
| `<Rate>` | 0=Disabled, 1=Every fix, N=Every N fixes (1–20) |
| `<MsgVer>` | Message version (optional, omit for standard NMEA) |

**Response:**
```
# Success:
$PQTMCFGMSGRATE,OK*29

# Error:
$PQTMCFGMSGRATE,ERROR,<ErrCode>*<Checksum>
# ErrCode: 1=Invalid params, 2=Execute failed
```

**Example:**
```
$PQTMCFGMSGRATE,W,PQTMEPE,1,2*1D
$PQTMCFGMSGRATE,OK*29
```

---

### PQTMEPE — Estimated Positioning Error

**Type:** Output

```
$PQTMEPE,<MsgVer>,<EPE_North>,<EPE_East>,<EPE_Down>,<EPE_2D>,<EPE_3D>*<Checksum>
```

| Field | Format | Unit | Description |
|-------|--------|------|-------------|
| `<MsgVer>` | numeric | — | Always 2 |
| `<EPE_North>` | n.xxx | meters | Estimated north error |
| `<EPE_East>` | n.xxx | meters | Estimated east error |
| `<EPE_Down>` | n.xxx | meters | Estimated down error |
| `<EPE_2D>` | n.xxx | meters | Estimated 2D position error |
| `<EPE_3D>` | n.xxx | meters | Estimated 3D position error |

**Example:**
```
$PQTMEPE,2,1.000,1.000,1.000,1.414,1.732*52
```

---

### PQTMSAVEPAR — Save PQTM Config to NVDM

**Type:** Command

```
$PQTMSAVEPAR*5A
→ $PQTMSAVEPAR,OK*72
→ $PQTMSAVEPAR,ERROR,<ErrCode>*<Checksum>
```

---

### PQTMRESTOREPAR — Restore PQTM Defaults

**Type:** Command

```
$PQTMRESTOREPAR*13
→ $PQTMRESTOREPAR,OK*3B
→ $PQTMRESTOREPAR,ERROR,<ErrCode>*<Checksum>
```

---

### PQTMVERNO — Query Firmware Version

**Type:** Command

```
$PQTMVERNO*58
→ $PQTMVERNO,<VerStr>,<BuildDate>,<BuildTime>*<Checksum>
```

| Field | Format | Description |
|-------|--------|-------------|
| `<VerStr>` | string | Firmware version string |
| `<BuildDate>` | yyyy/mm/dd | Build date |
| `<BuildTime>` | hh:mm:ss | Build time |

**LC76G Example:**
```
$PQTMVERNO*58
$PQTMVERNO,LC76GABNR02A01S,2022/09/14,11:47:03*3D
```

---

## 6. RTCM Protocol

Supports RTCM Standard 10403.3 (Differential GNSS Services – Version 3).

### Supported RTCM3 Messages

| Msg Type | Mode | Message Name |
|----------|------|-------------|
| 1005 | Output | Stationary RTK Reference Station ARP |
| 1019 | Output | GPS Ephemerides |
| 1020 | Output | GLONASS Ephemerides |
| 1042 | Output | BDS Satellite Ephemeris Data |
| 1044 | Output | QZSS Ephemerides |
| 1046 | Output | Galileo I/NAV Satellite Ephemeris Data |
| 1074 | Output | GPS MSM4 |
| 1077 | Output | GPS MSM7 |
| 1084 | Output | GLONASS MSM4 |
| 1087 | Output | GLONASS MSM7 |
| 1094 | Output | Galileo MSM4 |
| 1097 | Output | Galileo MSM7 |
| 1114 | Output | QZSS MSM4 |
| 1117 | Output | QZSS MSM7 |
| 1124 | Output | BDS MSM4 |
| 1127 | Output | BDS MSM7 |

- `$PAIR432` controls MSM4/MSM7 (1074–1127) when corresponding constellation enabled
- `$PAIR434` controls Stationary ARP (1005)
- `$PAIR436` controls ephemeris (1019–1046) when corresponding constellation enabled

---

## 7. GNSS Satellite Numbering (NMEA)

| GNSS Type | System ID | Satellite ID | Signal ID |
|-----------|-----------|-------------|-----------|
| GPS | 1 | 1–32 | 1 = L1 C/A |
| GLONASS | 2 | 65–88 | 1 = L1 |
| Galileo | 3 | 1–36 | 7 = E1 |
| BDS | 4 | 1–63 | 1 = B1I, 3 = B1C |
| QZSS | 5 | 193–199 | — |
| SBAS | — | 33–51 | — |

---

## 8. Special Characters Notation

| Character | Meaning |
|-----------|---------|
| `<...>` | Parameter name (angle brackets do not appear in message) |
| `[...]` | Optional field (square brackets do not appear in message) |
| `{…}` | Repeated field (curly brackets do not appear in message) |
| Underline | Default setting |

---

## 9. Terms & Abbreviations

| Abbreviation | Description |
|-------------|-------------|
| 2D/3D | 2/3 Dimension |
| ACK | Acknowledgement |
| AIC | Active Interference Cancellation |
| ALP | Adaptive Low Power |
| BDS | BeiDou Navigation Satellite System |
| C/N0 | Carrier-to-Noise-Density Ratio |
| COG | Course over Ground |
| DGPS | Differential GPS |
| DOP | Dilution of Precision |
| EASY | Embedded Assist System |
| EGNOS | European Geostationary Navigation Overlay Service |
| EPO | Extended Prediction Orbit |
| FLP | Fitness Low Power |
| GAGAN | GPS Aided GEO Augmented Navigation |
| GLP | GPS Low Power |
| GNSS | Global Navigation Satellite System |
| GPS | Global Positioning System |
| HDOP | Horizontal DOP |
| MNL | MTK Navigation Library |
| MSAS | Multi-functional Satellite Augmentation System |
| NMEA | National Marine Electronics Association 0183 |
| NVDM | Non-volatile Data Memory |
| NVRAM | Non-Volatile Random Access Memory |
| PAIR | Proprietary Protocol of MTK |
| PDOP | Position DOP |
| PPS | Pulse Per Second |
| QZSS | Quasi-Zenith Satellite System |
| RAIM | Receiver Autonomous Integrity Monitoring |
| RLM | Return Link Message |
| RMS | Root Mean Square |
| RTC | Real-time Clock |
| RTCM | Radio Technical Commission for Maritime Services |
| RTK | Real Time Kinematic |
| SBAS | Satellite-Based Augmentation System |
| SNR | Signal-to-noise Ratio |
| SOG | Speed over Ground |
| SPS | Standard Positioning Service |
| SV | Satellites in View |
| UART | Universal Asynchronous Receiver/Transmitter |
| UTC | Coordinated Universal Time |
| VDOP | Vertical DOP |
| WAAS | Wide Area Augmentation System |

---

## 10. Quick Reference — LC76G-Specific Notes

| Feature | LC76G (AB) | LC76G (PA) | LC76G (PB) |
|---------|-----------|-----------|-----------|
| Frequency bands | GPS+GLONASS+Galileo+BDS+QZSS L1 | Same | Same |
| PAIR050 (fix rate) | Supported (100–1000 ms) | **NOT supported** (1 Hz fixed) | **NOT supported** (1 Hz fixed) |
| GNS message (NMEA) | See version note below | Same | Same |
| RLM message | See version note below | Same | Same |
| PAIR158 (BDS B1C) | From PA/NR02A02S+ | From PA/NR02A02S+ | From PB/NR02A02S+ |
| EASY (PAIR490) | Supported | Supported | Supported |
| LOCUS (PAIR900) | From ABR02A01S+ | From PAR02A01S+ | From PBR02A01S+ |
| ALP (PAIR732) | From ABR02A01S+ | — | — |
| FLP (PAIR730) | Supported | Supported | Supported |
| GLP (PAIR680) | Supported | Supported | Supported |

**Firmware version boundaries:**
- `LC76GABNR02A01` / `LC76GABNR02A01S` — key cutoff for GNS, RLM, and several features
- `LC76GPANR02A02S` / `LC76GPBNR02A02S` — BDS B1C support added

---

*Generated from Quectel GNSS Protocol Specification V1.1 (2022-12-20)*
