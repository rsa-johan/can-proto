type Connection:
.connected 1
.disconnected 0

def {0}StatusId:
.nil

def {1}ConnectionStatus:
.connected 1 Connection

def {2,3}TemperatureError:
[#2]
.temp0 0
.temp1 1
.temp2 2
.temp3 3
.temp4 4
.temp5 5
.temp6 6
.temp7 7
[#3]
.temp8 0

frame CoprocStatus:
.ConnectionStatus
.TemperatureError