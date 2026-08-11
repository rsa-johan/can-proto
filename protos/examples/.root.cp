type NodeId:
.MC 0x01
.Coproc 0x02
.ConsoleRadio 0x03

type Priority:
.High 0x00
.Medium 0x01
.Low 0x02

framedef root:
.id '3 NodeId | '2 Priority