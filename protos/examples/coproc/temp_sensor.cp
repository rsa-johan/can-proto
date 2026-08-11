#class=temperature

type TemperatureSensorId:
.sensor_0 0
.sensor_1 1
.sensor_2 2
.sensor_3 3
.sensor_4 4

type TemperatureState:
.high 0
.low 1

def {0} TemperatureReqByte:
.temp_sensor_no 0..2 TemperatureSensorId

def {0} TemperatureResByte
.temp_sensor_val 0 TemperatureState

frame TemperatureRequest:
.size '1
.TemperatureReqByte

frame TemperatureResponse:
.size '1
.TemperatureResByte

rpc GetTemperature(TemperatureRequest) -> TemperatureResponse;