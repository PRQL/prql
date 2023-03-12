namespace Prql.Compiler
{
    internal struct NativePrqlCompilerOptions
    {
        public bool Format;
        public string Target;
        public bool SignatureComment;

        public NativePrqlCompilerOptions(PrqlCompilerOptions options)
        {
            Format = options.Format;
            Target = options.Target;
            SignatureComment = options.SignatureComment;
        }
    }
}
