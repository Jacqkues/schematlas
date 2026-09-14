"use client";
import { ShaderGradient, ShaderGradientCanvas } from "@shadergradient/react";
// Analogous mint, turquoise, and blue keep the evolving blends harmonious.
// Grain is composited once in CSS, independent of the moving shader.
export default function Shader() {
  return (
    <ShaderGradientCanvas
      style={{ position: "absolute", inset: 0 }}
      pixelDensity={1}
      fov={45}
    >
      <ShaderGradient
        control="props"
        type="waterPlane"
        animate="on"
        color1="#6cebbb"
        color2="#7daefa"
        color3="#45d6e9"
        uSpeed={0.22}
        uStrength={2.7}
        uDensity={1.05}
        uFrequency={2.6}
        uAmplitude={0}
        uTime={8}
        rotationX={0}
        rotationY={0}
        rotationZ={215}
        positionX={-0.5}
        positionY={0.1}
        cAzimuthAngle={180}
        cPolarAngle={115}
        cDistance={3.9}
        lightType="3d"
        brightness={1.2}
        reflection={0.01}
        grain="off"
      />
    </ShaderGradientCanvas>
  );
}
