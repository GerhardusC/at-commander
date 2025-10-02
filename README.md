How to send MQTT  publish using AT commands:

AT+CIPSTART="TCP","192.168.0.X",1883

AT+CIPSEND=21
10 13 00 04 4D 51 54 54 04 02 00 3C 00 07 63 6C 69 65 6E 74 31

AT+CIPSEND=20
30 12 00 0B 2F 74 65 73 74 2F 74 6F 70 69 63 68 65 6C 6C 6F

AT+CIPSEND=2
E0 00

1. Connect packet:

10: Fixed header -> CONNECT,

13: Remaining Length

00 04: Protocol name length

4D 51 54 54 (MQTT): ( Protocol -> (MQTT 3.1.1)

04: Protocol level

02: binary 00000010 (CleanSession = 1) (Will Flag = 0) (Will QoS = 00) (Will Retain = 0) (Password Flag = 0)

00 3C: Keepalive (60 seconds)

00 07: Client ID Length

63 6C 69 65 6E 74 31 ("client1"): ClientID

2. Publish packet:

30 → Fixed header: PUBLISH, QoS0, no flags

12 → Remaining length = 18

00 0B → Topic length = 11

2F 74 65 73 74 2F 74 6F 70 69 63 → Topic /test/topic

68 65 6C 6C 6F → Payload "hello"
