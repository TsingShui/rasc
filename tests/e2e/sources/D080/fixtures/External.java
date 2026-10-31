package fixtures;

/**
 * Independent Java classfile evidence for the D080 "known-constant" catalog
 * variant. javac --release 8 -proc:none -g:none inlines this field at use sites
 * and records a ConstantValue attribute on the field, which is the
 * JavaConstantKind=Constant proof. This class is not part of the DEX input: the
 * DEX only references Lfixtures/External;->C:I, and the "unknown-constantness"
 * variant deliberately omits this proof.
 */
public class External {
    public static final int C = 1;
}
