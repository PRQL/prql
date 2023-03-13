using System;

namespace Prql.Compiler
{
    internal struct NativeResult
    {
#pragma warning disable CS2141
        public string Output;
        public IntPtr Messages;
        public int MessagesLen;
#pragma warning restore CS2141
    }
}
