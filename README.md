## MQTT Packet Shapes Cheat Sheet

### 1. CONNECT (Client → Broker)

| Field                | Size (bytes) | Notes                                          |
| -------------------- | ------------ | ---------------------------------------------- |
| Fixed Header         | 1            | `0x10`                                         |
| Remaining Length     | 1–4          | All bytes below                                |
| Protocol Name Length | 2            | `0x00 0x04`                                    |
| Protocol Name        | 4            | `"MQTT"`                                       |
| Protocol Level       | 1            | `0x04` (MQTT v3.1.1)                           |
| Connect Flags        | 1            | Bitfield (Clean Session, Will, Username, etc.) |
| Keep Alive           | 2            | Seconds, big-endian                            |
| Client ID Length     | 2            | Length of client ID                            |
| Client ID            | N            | UTF-8 client identifier                        |

---

### 2. CONNACK (Broker → Client)

| Field                     | Size (bytes) | Notes                        |
| ------------------------- | ------------ | ---------------------------- |
| Fixed Header              | 1            | `0x20`                       |
| Remaining Length          | 1            | Always `0x02`                |
| Connect Acknowledge Flags | 1            | Bit 0 = Session Present      |
| Connect Return Code       | 1            | `0x00` = Connection Accepted |

---

### 3. PUBLISH (Client → Broker, QoS 0)

| Field            | Size (bytes) | Notes                                   |
| ---------------- | ------------ | --------------------------------------- |
| Fixed Header     | 1            | `0x30` = PUBLISH, QoS0, DUP=0, Retain=0 |
| Remaining Length | 1–4          | Covers everything below                 |
| Topic Length     | 2            | Big-endian                              |
| Topic Name       | N            | UTF-8 string                            |
| Payload          | M            | Application data                        |

**Formula:**

```
Remaining Length = 2 + N (topic) + M (payload)
```

---

### 4. DISCONNECT (Client → Broker)

| Field            | Size (bytes) | Notes         |
| ---------------- | ------------ | ------------- |
| Fixed Header     | 1            | `0xE0`        |
| Remaining Length | 1            | Always `0x00` |

**Buffer:**

```
E0 00
```

---

With these four packet shapes (CONNECT, CONNACK, PUBLISH, DISCONNECT), we have a complete minimal MQTT client lifecycle documented for connecting, publishing, and closing cleanly.

## How to Send MQTT Publish Using AT Commands

### 1. Open TCP connection to broker

```
AT+CIPSTART="TCP","192.168.0.X",1883
```

### 2. Send CONNECT packet

```
AT+CIPSEND=21
10 13 00 04 4D 51 54 54 04 02 00 3C 00 07 63 6C 69 65 6E 74 31
```

### 3. Send PUBLISH packet

```
AT+CIPSEND=20
30 12 00 0B 2F 74 65 73 74 2F 74 6F 70 69 63 68 65 6C 6C 6F
```

### 4. Send DISCONNECT packet

```
AT+CIPSEND=2
E0 00
```

## Packet breakdown

### 1. CONNECT packet

* `10`: Fixed header -> CONNECT
* `13`: Remaining Length
* `00 04`: Protocol name length
* `4D 51 54 54` (MQTT): Protocol name (MQTT 3.1.1)
* `04`: Protocol level
* `02`: Flags (CleanSession=1, Will=0, Password=0, Username=0)
* `00 3C`: Keepalive (60s)
* `00 07`: Client ID length
* `63 6C 69 65 6E 74 31` ("client1"): Client ID

### 2. PUBLISH packet

* `30`: Fixed header (PUBLISH, QoS0, no flags)
* `12`: Remaining length = 18
* `00 0B`: Topic length = 11
* `2F 74 65 73 74 2F 74 6F 70 69 63`: Topic `/test/topic`
* `68 65 6C 6C 6F`: Payload "hello"

This describes the AT commands required to connect to an MQTT broker, publish a message, and then disconnect using raw MQTT packets over an ESP/AT module.

