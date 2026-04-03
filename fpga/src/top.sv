`timescale 1ns / 1ps

module top_ascon_test (
    input  logic clk,       
    input  logic rst_n,       
    output logic [7:0] led_n  
);

    // --------------------------------------------------------
    // Power-On-Reset (POR) & Button Debouncer
    // --------------------------------------------------------
    logic [7:0] por_cnt = 0;
    logic sys_rst = 1; 

    always_ff @(posedge clk) begin
        if (!rst_n) begin
            por_cnt <= 0;
            sys_rst <= 1;
        end else if (por_cnt < 8'hFF) begin
            por_cnt <= por_cnt + 1;
            sys_rst <= 1;
        end else begin
            sys_rst <= 0;
        end
    end

    // --------------------------------------------------------
    // LWC Ascon Core Interface
    // --------------------------------------------------------
    logic [31:0] sdi_data; logic sdi_valid; logic sdi_ready;
    logic [31:0] pdi_data; logic pdi_valid; logic pdi_ready;
    logic [31:0] do_data;  logic do_valid;  logic do_ready;

    LWC u_ascon_core (
        .clk(clk),
        .rst(sys_rst), 
        .sdi_data(sdi_data), .sdi_valid(sdi_valid), .sdi_ready(sdi_ready),
        .pdi_data(pdi_data), .pdi_valid(pdi_valid), .pdi_ready(pdi_ready),
        .do_data(do_data),   .do_valid(do_valid),   .do_ready(do_ready)
    );

    // --------------------------------------------------------
    // Test Vectors
    // --------------------------------------------------------
    logic [31:0] sdi_seq [0:5] = '{
        32'h40000000, 32'hC7000010, // INS: Load Key, HDR: 16 bytes
        32'h01234567, 32'h89ABCDEF, 32'hFEDCBA98, 32'h76543210
    };

    logic [31:0] pdi_enc_seq [0:11] = '{
        32'h20000000, // [0] INS: Auth Encrypt
        32'hD2000010, // [1] HDR: Npub 16 bytes
        32'h00000000, 32'h00000000, 32'h00000000, 32'h00000000,
        32'h12000000, // [6] HDR: AD 0 bytes
        32'h47000010, // [7] HDR: PT 16 bytes
        32'hDEADBEEF, 32'hCAFEBABE, 32'h11223344, 32'h55667788 // PT[8:11]
    };

    logic [31:0] ct_buffer [0:3]; 
    logic [31:0] tag_buffer [0:3];
    logic [31:0] pt_buffer [0:3];
    logic [31:0] enc_status, dec_status;

    // --------------------------------------------------------
    // Output Sink (Concurrent Capture Block)
    // --------------------------------------------------------
    // Never stall the output! Always be ready to catch data.
    assign do_ready = 1'b1; 

    int out_idx = 0;
    logic is_decrypting = 0;
    logic enc_done = 0;
    logic dec_done = 0;

    always_ff @(posedge clk) begin
        if (sys_rst) begin
            out_idx <= 0;
            enc_done <= 0;
            dec_done <= 0;
        end else if (do_valid && do_ready) begin
            if (!is_decrypting) begin
                // --- Catching Encryption Output (11 Words) ---
                if      (out_idx >= 1 && out_idx <= 4) ct_buffer[out_idx-1] <= do_data;
                else if (out_idx >= 6 && out_idx <= 9) tag_buffer[out_idx-6] <= do_data;
                else if (out_idx == 10) begin 
                    enc_status <= do_data; // 0xE0000000
                    enc_done <= 1;         // Signal FSM we are done
                end
                out_idx <= (out_idx == 10) ? 0 : out_idx + 1;
            end else begin
                // --- Catching Decryption Output (6 Words) ---
                if      (out_idx >= 1 && out_idx <= 4) pt_buffer[out_idx-1] <= do_data;
                else if (out_idx == 5) begin
                    dec_status <= do_data; // 0xE0000000
                    dec_done <= 1;         // Signal FSM we are done
                end
                out_idx <= (out_idx == 5) ? 0 : out_idx + 1;
            end
        end
    end

    // --------------------------------------------------------
    // Input Feeder (Main FSM)
    // --------------------------------------------------------
    typedef enum logic [3:0] {
        ST_INIT        = 4'd0,
        ST_ACTKEY_FEED = 4'd1, 
        ST_LOAD_KEY    = 4'd2, 
        ST_ENC_FEED    = 4'd3, 
        ST_ENC_WAIT    = 4'd4, 
        ST_DEC_FEED    = 4'd5, 
        ST_DEC_WAIT    = 4'd6, 
        ST_VERIFY      = 4'd7,
        ST_HALT        = 4'd8
    } state_t;

    state_t state;
    int idx;
    logic [7:0] debug_leds;

    assign led_n = ~debug_leds; 

    // --- Combinational Data Routing ---
    always_comb begin
        pdi_valid = 1'b0; pdi_data = 32'h00000000;
        sdi_valid = 1'b0; sdi_data = 32'h00000000;
        
        case (state)
            ST_ACTKEY_FEED: begin
                pdi_valid = 1'b1;
                pdi_data  = 32'h70000000; 
            end
            ST_LOAD_KEY: begin
                sdi_valid = 1'b1;
                sdi_data  = sdi_seq[idx];
            end
            ST_ENC_FEED: begin
                pdi_valid = 1'b1;
                pdi_data  = pdi_enc_seq[idx];
            end
            ST_DEC_FEED: begin
                pdi_valid = 1'b1;
                // Dynamically build the 17-word decryption stream
                case (idx)
                    0: pdi_data = 32'h30000000; // Auth Decrypt INS
                    1: pdi_data = 32'hD2000010; // Npub HDR
                    2,3,4,5: pdi_data = 32'h00000000; // Npub
                    6: pdi_data = 32'h12000000; // AD HDR (0)
                    7: pdi_data = 32'h56000010; // CT HDR
                    8,9,10,11: pdi_data = ct_buffer[idx-8]; // Recirculate CT
                    12: pdi_data = 32'h83000010; // Tag HDR
                    13,14,15,16: pdi_data = tag_buffer[idx-13]; // Recirculate Tag
                    default: pdi_data = 32'h00000000;
                endcase
            end
            default: ;
        endcase
    end

    // --- State Transitions ---
    always_ff @(posedge clk) begin
        if (sys_rst) begin
            state <= ST_INIT;
            idx <= 0; 
            is_decrypting <= 0;
            debug_leds <= 8'h00;
        end else begin
            debug_leds[3:0] <= state; 

            case (state)
                ST_INIT: begin
                    idx <= 0; is_decrypting <= 0;
                    state <= ST_ACTKEY_FEED;
                end
                ST_ACTKEY_FEED: begin
                    if (pdi_ready && pdi_valid) state <= ST_LOAD_KEY;
                end
                ST_LOAD_KEY: begin
                    if (sdi_ready && sdi_valid) begin 
                        if (idx == 5) begin
                            idx <= 0;
                            state <= ST_ENC_FEED;
                        end else idx++;
                    end
                end
                ST_ENC_FEED: begin
                    if (pdi_ready && pdi_valid) begin 
                        if (idx == 11) begin 
                            idx <= 0;
                            state <= ST_ENC_WAIT;
                        end else idx++;
                    end
                end
                ST_ENC_WAIT: begin
                    // Wait for the concurrent output block to finish catching
                    if (enc_done) begin  
                        is_decrypting <= 1; // Tell Output Block to switch modes
                        state <= ST_DEC_FEED;
                    end
                end
                ST_DEC_FEED: begin
                    if (pdi_ready && pdi_valid) begin
                        if (idx == 16) begin 
                            idx <= 0;
                            state <= ST_DEC_WAIT;
                        end else idx++;
                    end
                end
                ST_DEC_WAIT: begin
                    // Wait for Decrypt Status Word
                    if (dec_done) begin 
                        state <= ST_VERIFY;
                    end
                end
                ST_VERIFY: begin
                    // 0xE0000000 is the LWC Auth Success code
                    if ((dec_status == 32'hE0000000) && 
                        (pt_buffer[0] == pdi_enc_seq[8]) && 
                        (pt_buffer[1] == pdi_enc_seq[9]) &&
                        (pt_buffer[2] == pdi_enc_seq[10]) && 
                        (pt_buffer[3] == pdi_enc_seq[11])) begin
                        debug_leds[7] <= 1'b1; // Success!
                    end else begin
                        debug_leds[6] <= 1'b1; // Auth or PT Mismatch 
                    end
                    state <= ST_HALT;
                end
                ST_HALT: ; 
            endcase
        end
    end
endmodule