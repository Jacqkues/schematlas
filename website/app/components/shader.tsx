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
        color1="#71dfb3"
        color2="#91aff3"
        color3="#62d2de"
        uSpeed={0.07}
        uStrength={2}
        uDensity={0.8}
        uFrequency={2.2}
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
        brightness={1.1}
        reflection={0.01}
        grain="off"
      />
    </ShaderGradientCanvas>
  );
}
