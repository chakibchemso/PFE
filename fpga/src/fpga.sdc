//Copyright (C)2014-2026 GOWIN Semiconductor Corporation.
//All rights reserved.
//File Title: Timing Constraints file
//Tool Version: V1.9.12.02 
//Created Time: 2026-03-30 10:49:49
create_clock -name clk -period 20 -waveform {0 10} [get_ports {clk}]
