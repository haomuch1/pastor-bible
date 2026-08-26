"""Canon tables and reference-abbreviation maps.

Book names, abbreviations and ordering are NOT hardcoded here: they are read
from the USFM files themselves (\\h, \\toc1, \\toc3) so that the database says
what the source says. What lives here is only the information the USFM does not
carry: which books belong to the protestant 66, which files are not scripture at
all, and how other corpora spell book references.
"""

# The 66 books of the protestant canon, by USFM book code. Everything else in
# the WEB ecumenical set is flagged canon='deutero'.
PROTESTANT_66 = [
    'GEN', 'EXO', 'LEV', 'NUM', 'DEU', 'JOS', 'JDG', 'RUT', '1SA', '2SA',
    '1KI', '2KI', '1CH', '2CH', 'EZR', 'NEH', 'EST', 'JOB', 'PSA', 'PRO',
    'ECC', 'SNG', 'ISA', 'JER', 'LAM', 'EZK', 'DAN', 'HOS', 'JOL', 'AMO',
    'OBA', 'JON', 'MIC', 'NAM', 'HAB', 'ZEP', 'HAG', 'ZEC', 'MAL',
    'MAT', 'MRK', 'LUK', 'JHN', 'ACT', 'ROM', '1CO', '2CO', 'GAL', 'EPH',
    'PHP', 'COL', '1TH', '2TH', '1TI', '2TI', 'TIT', 'PHM', 'HEB', 'JAS',
    '1PE', '2PE', '1JN', '2JN', '3JN', 'JUD', 'REV',
]
assert len(PROTESTANT_66) == 66

NEW_TESTAMENT = set(PROTESTANT_66[39:])
assert len(NEW_TESTAMENT) == 27

# USFM files that are not scripture and carry no verses.
NON_SCRIPTURE = {'FRT', 'BAK', 'GLO', 'INT', 'CNC', 'TDX', 'NDX', 'OTH', 'XXA',
                 'XXB', 'XXC', 'XXD', 'XXE', 'XXF', 'XXG'}

# OSIS book codes -> USFM code. Used for Nave's, whose TEI carries osisRef
# attributes. Only the protestant 66 appear in Nave's; deutero codes are
# included so that a reference into them resolves rather than silently failing.
OSIS_TO_USFM = {
    'Gen': 'GEN', 'Exod': 'EXO', 'Lev': 'LEV', 'Num': 'NUM', 'Deut': 'DEU',
    'Josh': 'JOS', 'Judg': 'JDG', 'Ruth': 'RUT', '1Sam': '1SA', '2Sam': '2SA',
    '1Kgs': '1KI', '2Kgs': '2KI', '1Chr': '1CH', '2Chr': '2CH', 'Ezra': 'EZR',
    'Neh': 'NEH', 'Esth': 'EST', 'Job': 'JOB', 'Ps': 'PSA', 'Prov': 'PRO',
    'Eccl': 'ECC', 'Song': 'SNG', 'Isa': 'ISA', 'Jer': 'JER', 'Lam': 'LAM',
    'Ezek': 'EZK', 'Dan': 'DAN', 'Hos': 'HOS', 'Joel': 'JOL', 'Amos': 'AMO',
    'Obad': 'OBA', 'Jonah': 'JON', 'Mic': 'MIC', 'Nah': 'NAM', 'Hab': 'HAB',
    'Zeph': 'ZEP', 'Hag': 'HAG', 'Zech': 'ZEC', 'Mal': 'MAL',
    'Matt': 'MAT', 'Mark': 'MRK', 'Luke': 'LUK', 'John': 'JHN', 'Acts': 'ACT',
    'Rom': 'ROM', '1Cor': '1CO', '2Cor': '2CO', 'Gal': 'GAL', 'Eph': 'EPH',
    'Phil': 'PHP', 'Col': 'COL', '1Thess': '1TH', '2Thess': '2TH',
    '1Tim': '1TI', '2Tim': '2TI', 'Titus': 'TIT', 'Phlm': 'PHM', 'Heb': 'HEB',
    'Jas': 'JAS', '1Pet': '1PE', '2Pet': '2PE', '1John': '1JN', '2John': '2JN',
    '3John': '3JN', 'Jude': 'JUD', 'Rev': 'REV',
    # Deuterocanon, OSIS names
    'Tob': 'TOB', 'Jdt': 'JDT', 'AddEsth': 'ESG', 'EsthGr': 'ESG',
    'Wis': 'WIS', 'Sir': 'SIR', 'Bar': 'BAR', 'EpJer': 'BAR',
    '1Macc': '1MA', '2Macc': '2MA', '3Macc': '3MA', '4Macc': '4MA',
    '1Esd': '1ES', '2Esd': '2ES', 'PrMan': 'MAN', 'AddPs': 'PS2',
    'Ps151': 'PS2', 'DanGr': 'DAG', 'AddDan': 'DAG',
    'Sus': 'DAG', 'Bel': 'DAG', 'PrAzar': 'DAG', 'SgThree': 'DAG',
}

# Treasury of Scripture Knowledge writes references in the compact KJV style
# ("Ge", "1Sa", "Joh", "Re"). Keys are lowercased and stripped of spaces and
# full stops before lookup, so "1 Sa." and "1Sa" both match.
TSK_ABBREV = {
    'ge': 'GEN', 'gen': 'GEN', 'genesis': 'GEN',
    'ex': 'EXO', 'exo': 'EXO', 'exod': 'EXO', 'exodus': 'EXO',
    'le': 'LEV', 'lev': 'LEV', 'leviticus': 'LEV',
    'nu': 'NUM', 'num': 'NUM', 'numbers': 'NUM',
    'de': 'DEU', 'deu': 'DEU', 'deut': 'DEU', 'deuteronomy': 'DEU',
    'jos': 'JOS', 'josh': 'JOS', 'joshua': 'JOS',
    'jud': 'JDG', 'jdg': 'JDG', 'judg': 'JDG', 'judges': 'JDG',
    'ru': 'RUT', 'rut': 'RUT', 'ruth': 'RUT',
    '1sa': '1SA', '1sam': '1SA', '1samuel': '1SA',
    '2sa': '2SA', '2sam': '2SA', '2samuel': '2SA',
    '1ki': '1KI', '1kg': '1KI', '1kings': '1KI',
    '2ki': '2KI', '2kg': '2KI', '2kings': '2KI',
    '1ch': '1CH', '1chr': '1CH', '1chronicles': '1CH',
    '2ch': '2CH', '2chr': '2CH', '2chronicles': '2CH',
    'ezr': 'EZR', 'ezra': 'EZR',
    'ne': 'NEH', 'neh': 'NEH', 'nehemiah': 'NEH',
    'es': 'EST', 'est': 'EST', 'esth': 'EST', 'esther': 'EST',
    'job': 'JOB',
    'ps': 'PSA', 'psa': 'PSA', 'psalm': 'PSA', 'psalms': 'PSA',
    'pr': 'PRO', 'pro': 'PRO', 'prov': 'PRO', 'proverbs': 'PRO',
    'ec': 'ECC', 'ecc': 'ECC', 'eccl': 'ECC', 'ecclesiastes': 'ECC',
    'so': 'SNG', 'sos': 'SNG', 'song': 'SNG', 'sng': 'SNG', 'cant': 'SNG',
    'isa': 'ISA', 'is': 'ISA', 'isaiah': 'ISA',
    'jer': 'JER', 'je': 'JER', 'jeremiah': 'JER',
    'la': 'LAM', 'lam': 'LAM', 'lamentations': 'LAM',
    'eze': 'EZK', 'ezek': 'EZK', 'ezk': 'EZK', 'ezekiel': 'EZK',
    'da': 'DAN', 'dan': 'DAN', 'daniel': 'DAN',
    'ho': 'HOS', 'hos': 'HOS', 'hosea': 'HOS',
    'joe': 'JOL', 'joel': 'JOL', 'jol': 'JOL',
    'am': 'AMO', 'amo': 'AMO', 'amos': 'AMO',
    'ob': 'OBA', 'oba': 'OBA', 'obad': 'OBA', 'obadiah': 'OBA',
    'jon': 'JON', 'jonah': 'JON',
    'mic': 'MIC', 'mi': 'MIC', 'micah': 'MIC',
    'na': 'NAM', 'nah': 'NAM', 'nam': 'NAM', 'nahum': 'NAM',
    'hab': 'HAB', 'habakkuk': 'HAB',
    'zep': 'ZEP', 'zeph': 'ZEP', 'zephaniah': 'ZEP',
    'hag': 'HAG', 'haggai': 'HAG',
    'zec': 'ZEC', 'zech': 'ZEC', 'zechariah': 'ZEC',
    'mal': 'MAL', 'malachi': 'MAL',
    'mt': 'MAT', 'mat': 'MAT', 'matt': 'MAT', 'matthew': 'MAT',
    'mr': 'MRK', 'mk': 'MRK', 'mar': 'MRK', 'mark': 'MRK', 'mrk': 'MRK',
    'lu': 'LUK', 'lk': 'LUK', 'luk': 'LUK', 'luke': 'LUK',
    'joh': 'JHN', 'jn': 'JHN', 'jhn': 'JHN', 'john': 'JHN',
    'ac': 'ACT', 'act': 'ACT', 'acts': 'ACT',
    'ro': 'ROM', 'rom': 'ROM', 'romans': 'ROM',
    '1co': '1CO', '1cor': '1CO', '1corinthians': '1CO',
    '2co': '2CO', '2cor': '2CO', '2corinthians': '2CO',
    'ga': 'GAL', 'gal': 'GAL', 'galatians': 'GAL',
    'eph': 'EPH', 'ephesians': 'EPH',
    'php': 'PHP', 'phi': 'PHP', 'phil': 'PHP', 'philippians': 'PHP',
    'col': 'COL', 'colossians': 'COL',
    '1th': '1TH', '1thes': '1TH', '1thess': '1TH', '1thessalonians': '1TH',
    '2th': '2TH', '2thes': '2TH', '2thess': '2TH', '2thessalonians': '2TH',
    '1ti': '1TI', '1tim': '1TI', '1timothy': '1TI',
    '2ti': '2TI', '2tim': '2TI', '2timothy': '2TI',
    'tit': 'TIT', 'titus': 'TIT',
    'phm': 'PHM', 'phile': 'PHM', 'philem': 'PHM', 'philemon': 'PHM',
    'heb': 'HEB', 'hebrews': 'HEB',
    'jas': 'JAS', 'jam': 'JAS', 'james': 'JAS',
    '1pe': '1PE', '1pet': '1PE', '1peter': '1PE',
    '2pe': '2PE', '2pet': '2PE', '2peter': '2PE',
    '1jo': '1JN', '1joh': '1JN', '1jn': '1JN', '1john': '1JN',
    '2jo': '2JN', '2joh': '2JN', '2jn': '2JN', '2john': '2JN',
    '3jo': '3JN', '3joh': '3JN', '3jn': '3JN', '3john': '3JN',
    'jude': 'JUD', 'jde': 'JUD',
    're': 'REV', 'rev': 'REV', 'revelation': 'REV',
}


def canon_of(usfm_code):
    return 'protestant' if usfm_code in PROTESTANT_66 else 'deutero'


def testament_of(usfm_code):
    """OT or NT.

    Every deuterocanonical book in the WEB ecumenical set belongs to the Old
    Testament era and is placed in the OT section, so it is recorded as OT.
    The protestant/deutero distinction is carried by the canon column, not by
    this one.
    """
    return 'NT' if usfm_code in NEW_TESTAMENT else 'OT'
