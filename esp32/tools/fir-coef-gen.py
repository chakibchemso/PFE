# pip install scipy
from scipy.signal import firwin

# 201 taps, Bandpass 0.5Hz - 3.5Hz, 100Hz sample rate
taps = firwin(201, [0.5, 3.5], pass_zero=False, fs=100)

print("pub const FIR_COEFFS: [f32; 201] = [")
for t in taps:
    print(f"    {t}f32,")
print("];")
