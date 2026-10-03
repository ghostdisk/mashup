//! Small source-observed operations used by GTA's on-foot task.
//! See docs/gtasa-mechanics.md for this Steam executable's Ghidra evidence.
//! Physical movement, animation root motion and task dispatch are separate work.

/// GTA's normalized update factor: elapsed milliseconds / 20, not seconds.
/// This models the ordinary timer path with the extra subminimum-step flag off.
#[derive(Clone, Copy, Debug)]
pub struct GtaTimeStep(f32);
impl GtaTimeStep {
    pub fn from_elapsed_ms(elapsed_ms:f32,time_scale:f32,paused:bool)->Option<Self> {
        if !elapsed_ms.is_finite() || elapsed_ms<0.0 || !time_scale.is_finite() || time_scale<0.0 {return None;}
        let scaled=if paused {0.0}else{elapsed_ms*time_scale};
        if !scaled.is_finite() {return None;}
        let factor=scaled/20.0;
        Some(Self(if paused {factor.clamp(0.00001,3.0)}else{factor.clamp(0.01,3.0)}))
    }
    pub fn factor(self)->f32 {self.0}
}

/// The scalar movement blend written by the task's normal movement branch,
/// before the downstream animation/sprint update can alter it.
/// It is not world velocity or a complete GTA controller.
#[derive(Clone, Copy, Debug, Default)]
pub struct NormalMoveBlend {pub amount:f32}
impl NormalMoveBlend {
    /// Axes are the source signed pad values, after pad-disable/mode handling.
    /// `limit_to_one` corresponds to the observed pad short at offset 0x2a;
    /// the caller supplies the physical/direction eligibility decision.
    pub fn update(&mut self,axes:[i16;2],limit_to_one:bool,movement_allowed:bool,step:GtaTimeStep)->f32 {
        let x=f32::from(axes[0]);let y=f32::from(axes[1]);
        let mut target=(x*x+y*y).sqrt()/60.0;
        if limit_to_one {target=target.min(1.0);}
        if target<=0.0 || !movement_allowed {self.amount=0.0;return self.amount;}
        // Source multiplies the float timestep by this double, then stores float.
        let change=(f64::from(step.factor())*0.07000000029802322) as f32;
        self.amount=if target-self.amount>change {self.amount+change}
            else if target-self.amount < -change {self.amount-change}else{target};
        self.amount
    }
}
