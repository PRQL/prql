using System;

namespace Prql.Compiler
{
    internal struct NativeResult
    {
#pragma warning disable CS0649
        public string Output;
        public IntPtr Messages;
        public int MessagesLen;
#pragma warning restore CS0649
    }
}
